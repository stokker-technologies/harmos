// SPDX-License-Identifier: Apache-2.0

// Package sidecar implements the bounded Go review slice of the sidecar protocol.
package sidecar

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"sync"
	"sync/atomic"

	"github.com/stokker-technologies/harmos/sdk/go/wire"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/proto"
)

// Operation declares the only application operation in this review slice.
// Run owns its operation policy and must return when ctx is cancelled.
type Operation struct {
	Owner string
	Name  string
	Run   func(ctx context.Context, payload []byte) ([]byte, error)
}

// Settings are consumer policy; the SDK does not choose resource budgets.
type Settings struct {
	MessageBytes          int
	ConcurrentRPCs        uint32
	StreamWindowBytes     int32
	ConnectionWindowBytes int32
}

type invocation struct {
	operation string
	payload   []byte
	phase     uint32
	cancel    context.CancelFunc
	ctx       context.Context
	done      chan struct{}
}

type flight struct {
	id       uint64
	cancel   chan struct{}
	once     sync.Once
	done     chan struct{}
	produced atomic.Uint64
}

type server struct {
	wire.UnimplementedSidecarServer
	operation    Operation
	token        string
	generation   uint64
	mu           sync.Mutex
	initialized  bool
	retired      bool
	invocation   *invocation
	invocationID uint64
	flight       *flight
	stop         chan struct{}
}

// Serve consumes the host's private stdin bootstrap and advertises only the
// bound loopback address on stdout. The host owns abnormal process teardown.
func Serve(input io.Reader, output io.Writer, operation Operation, settings Settings) error {
	if operation.Owner == "" || operation.Name == "" || operation.Run == nil {
		return fmt.Errorf("owner, operation name and handler required")
	}
	if settings.MessageBytes <= 0 || settings.ConcurrentRPCs < 2 || settings.StreamWindowBytes < 65535 || settings.ConnectionWindowBytes < 65535 {
		return fmt.Errorf("positive message budget, two concurrent RPCs and supported HTTP/2 windows required")
	}

	// Receive Private Generation Credentials.
	var bootstrap []json.RawMessage
	if err := json.NewDecoder(io.LimitReader(input, int64(settings.MessageBytes))).Decode(&bootstrap); err != nil {
		return err
	}
	s := &server{operation: operation, stop: make(chan struct{})}
	if len(bootstrap) != 2 {
		return fmt.Errorf("bootstrap must contain token and generation")
	}
	if err := json.Unmarshal(bootstrap[0], &s.token); err != nil {
		return err
	}
	if err := json.Unmarshal(bootstrap[1], &s.generation); err != nil {
		return err
	}
	credential, err := hex.DecodeString(s.token)
	if err != nil || len(credential) != 32 || s.generation == 0 {
		return fmt.Errorf("256-bit token and positive generation required")
	}

	// Bind And Register Before Advertising Readiness.
	listener, err := net.Listen("tcp4", "127.0.0.1:0")
	if err != nil {
		return err
	}
	defer listener.Close()
	transport := grpc.NewServer(
		grpc.MaxRecvMsgSize(settings.MessageBytes), grpc.MaxSendMsgSize(settings.MessageBytes),
		grpc.MaxConcurrentStreams(settings.ConcurrentRPCs),
		grpc.StaticStreamWindowSize(settings.StreamWindowBytes),
		grpc.StaticConnWindowSize(settings.ConnectionWindowBytes),
	)
	defer transport.Stop()
	wire.RegisterSidecarServer(transport, s)
	if _, err := fmt.Fprintln(output, listener.Addr()); err != nil {
		return err
	}
	serving := make(chan error, 1)
	go func() { serving <- transport.Serve(listener) }()
	select {
	case err := <-serving:
		return err
	case <-s.stop:
		transport.GracefulStop()
		if s.flight != nil {
			<-s.flight.done
		}
		return <-serving
	}
}

func (s *server) admit(ctx context.Context, packet *wire.Packet) error {
	credentials := metadata.ValueFromIncomingContext(ctx, "authorization")
	if len(credentials) != 1 || credentials[0] != s.token {
		return status.Error(codes.Unauthenticated, "generation credentials refused")
	}
	if s.retired || packet.Generation != s.generation || packet.Version != uint32(wire.Protocol_PROTOCOL_VERSION) {
		return status.Error(codes.FailedPrecondition, "version or generation refused")
	}
	return nil
}

func (s *server) Control(ctx context.Context, p *wire.Packet) (*wire.Packet, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.admit(ctx, p); err != nil {
		return nil, err
	}
	result := &wire.Packet{}
	switch wire.Phase(p.Phase) {
	case wire.Phase_PHASE_WELCOME:
		result.Fingerprint = wire.SchemaFingerprint[:]
		result.Context = wire.SchemaFingerprint[:]
		result.Declarations = []*wire.Declaration{{Operation: s.operation.Owner + "." + s.operation.Name, Fingerprint: wire.SchemaFingerprint[:]}}
	case wire.Phase_PHASE_PREPARE:
		if s.invocation != nil {
			return nil, status.Error(codes.ResourceExhausted, "one control invocation at a time")
		}
		lifecycle := p.Operation == s.operation.Owner+".harmos.initialize" || p.Operation == s.operation.Owner+".harmos.shutdown"
		declared := p.Operation == s.operation.Owner+"."+s.operation.Name
		if !lifecycle && !declared {
			return nil, status.Error(codes.Unimplemented, "operation not declared")
		}
		if p.Invocation == 0 || len(p.Context) != 32 || !bytes.Equal(p.Fingerprint, wire.SchemaFingerprint[:]) {
			return nil, status.Error(codes.InvalidArgument, "invocation, context or fingerprint refused")
		}
		if declared && !s.initialized {
			return nil, status.Error(codes.FailedPrecondition, "plugin not initialized")
		}
		callCtx, cancel := context.WithCancel(context.Background())
		s.invocation = &invocation{operation: p.Operation, payload: p.Payload, phase: p.Phase, ctx: callCtx, cancel: cancel, done: make(chan struct{})}
		s.invocationID = p.Invocation
	case wire.Phase_PHASE_INITIALIZE, wire.Phase_PHASE_ATTEMPT, wire.Phase_PHASE_FINALIZE, wire.Phase_PHASE_CANCEL:
		call := s.invocation
		if call == nil || s.invocationID != p.Invocation {
			return nil, status.Error(codes.NotFound, "invocation missing")
		}
		switch wire.Phase(p.Phase) {
		case wire.Phase_PHASE_INITIALIZE:
			if call.phase != uint32(wire.Phase_PHASE_PREPARE) {
				return nil, status.Error(codes.FailedPrecondition, "prepare required")
			}
			call.phase = p.Phase
		case wire.Phase_PHASE_ATTEMPT:
			if call.phase != uint32(wire.Phase_PHASE_INITIALIZE) {
				return nil, status.Error(codes.FailedPrecondition, "initialize required")
			}
			call.phase = p.Phase
			// Release Ownership Lock While The Operation Runs.
			s.mu.Unlock()
			payload, err := s.attempt(call)
			s.mu.Lock()
			close(call.done)
			result.Payload = payload
			return result, err
		case wire.Phase_PHASE_CANCEL:
			call.cancel()
		case wire.Phase_PHASE_FINALIZE:
			if call.phase == uint32(wire.Phase_PHASE_ATTEMPT) {
				select {
				case <-call.done:
				default:
					return nil, status.Error(codes.FailedPrecondition, "attempt still running")
				}
			}
			call.cancel()
			s.invocation = nil
		}
	case wire.Phase_PHASE_CANCEL_DATA, wire.Phase_PHASE_PROGRESS:
		f := s.flight
		if f == nil || f.id != p.Invocation {
			return nil, status.Error(codes.NotFound, "Arrow invocation missing")
		}
		if wire.Phase(p.Phase) == wire.Phase_PHASE_CANCEL_DATA {
			f.once.Do(func() { close(f.cancel) })
			// Settlement Never Needs The Receiver To Drain Buffered Data.
			s.mu.Unlock()
			select {
			case <-f.done:
			case <-ctx.Done():
				s.mu.Lock()
				return nil, status.FromContextError(ctx.Err()).Err()
			}
			s.mu.Lock()
		}
		result.Count = f.produced.Load()
		select {
		case <-f.done:
			result.Phase = 1
		default:
		}
	case wire.Phase_PHASE_RETIRE:
		s.retired = true
		if s.invocation != nil {
			s.invocation.cancel()
		}
		if s.flight != nil {
			s.flight.once.Do(func() { close(s.flight.cancel) })
		}
		close(s.stop)
	default:
		return nil, status.Error(codes.Unimplemented, "prototype phase")
	}
	return result, nil
}

func (s *server) attempt(call *invocation) ([]byte, error) {
	switch call.operation {
	case s.operation.Owner + ".harmos.initialize":
		var request wire.Initialization
		if err := proto.Unmarshal(call.payload, &request); err != nil {
			return nil, status.Error(codes.InvalidArgument, err.Error())
		}
		if !json.Valid(request.ConfigJson) || len(request.GenerationContext) != 32 || len(request.Assignments) != 0 {
			return nil, status.Error(codes.InvalidArgument, "valid JSON, generation context and no host assignments required")
		}
		s.mu.Lock()
		s.initialized = true
		s.mu.Unlock()
		return nil, nil
	case s.operation.Owner + ".harmos.shutdown":
		s.mu.Lock()
		s.initialized = false
		s.mu.Unlock()
		return nil, nil
	default:
		return s.operation.Run(call.ctx, call.payload)
	}
}

func (s *server) Data(p *wire.Packet, stream grpc.ServerStreamingServer[wire.Packet]) error {
	s.mu.Lock()
	if err := s.admit(stream.Context(), p); err != nil {
		s.mu.Unlock()
		return err
	}
	if p.Invocation == 0 || len(p.Payload) == 0 || p.Count == 0 || !s.initialized {
		s.mu.Unlock()
		return status.Error(codes.InvalidArgument, "initialized plugin, invocation, Arrow IPC and positive count required")
	}
	if s.flight != nil {
		select {
		case <-s.flight.done:
		default:
			s.mu.Unlock()
			return status.Error(codes.ResourceExhausted, "one Arrow invocation at a time")
		}
	}
	f := &flight{id: p.Invocation, cancel: make(chan struct{}), done: make(chan struct{})}
	s.flight = f
	s.mu.Unlock()

	// One Sender, No Application Queue Or Per-Batch Receipts.
	finished := make(chan error, 1)
	go func() {
		defer close(f.done)
		for sequence := uint64(0); sequence < p.Count; sequence++ {
			select {
			case <-f.cancel:
				return
			case <-stream.Context().Done():
				return
			default:
			}
			if err := stream.Send(&wire.Packet{Invocation: p.Invocation, Count: sequence, Payload: p.Payload}); err != nil {
				finished <- err
				return
			}
			f.produced.Add(1)
		}
		finished <- nil
	}()
	// Returning The RPC Cancels A Send Blocked Inside gRPC's HTTP/2 Flow Control.
	select {
	case err := <-finished:
		return err
	case <-f.cancel:
		return status.Error(codes.Canceled, "Arrow invocation cancelled")
	case <-stream.Context().Done():
		return status.FromContextError(stream.Context().Err()).Err()
	}
}
