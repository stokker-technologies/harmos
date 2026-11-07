// SPDX-License-Identifier: Apache-2.0

package sidecar

import (
	"context"
	"io"
	"strings"
	"testing"

	"github.com/stokker-technologies/harmos/sdk/go/wire"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/metadata"
	"google.golang.org/grpc/status"
	"google.golang.org/protobuf/proto"
)

func TestAdmission(t *testing.T) {
	s := &server{token: "secret", generation: 9}
	for _, test := range []struct {
		name       string
		token      string
		version    uint32
		generation uint64
		retired    bool
		code       codes.Code
	}{
		{"accepted", "secret", 1, 9, false, codes.OK},
		{"credentials", "wrong", 1, 9, false, codes.Unauthenticated},
		{"version", "secret", 2, 9, false, codes.FailedPrecondition},
		{"generation", "secret", 1, 8, false, codes.FailedPrecondition},
		{"retirement", "secret", 1, 9, true, codes.FailedPrecondition},
	} {
		t.Run(test.name, func(t *testing.T) {
			s.retired = test.retired
			ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs("authorization", test.token))
			err := s.admit(ctx, &wire.Packet{Version: test.version, Generation: test.generation})
			if status.Code(err) != test.code {
				t.Fatalf("admission code = %v, want %v", status.Code(err), test.code)
			}
		})
	}
}

func TestInvocationOrderingAndCancellation(t *testing.T) {
	started := make(chan struct{})
	s := &server{
		token: "secret", generation: 9, initialized: true,
		operation: Operation{Owner: "test", Name: "greet", Run: func(ctx context.Context, _ []byte) ([]byte, error) {
			close(started)
			<-ctx.Done()
			return nil, status.FromContextError(ctx.Err()).Err()
		}},
	}
	ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs("authorization", s.token))
	prepare := &wire.Packet{
		Version: 1, Generation: 9, Phase: uint32(wire.Phase_PHASE_PREPARE), Invocation: 5,
		Operation: "test.greet", Fingerprint: wire.SchemaFingerprint[:], Context: make([]byte, 32),
	}
	if _, err := s.Control(ctx, prepare); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Control(ctx, prepare); status.Code(err) != codes.ResourceExhausted {
		t.Fatalf("overlapping prepare: %v", err)
	}
	attempt := &wire.Packet{Version: 1, Generation: 9, Invocation: 5, Phase: uint32(wire.Phase_PHASE_ATTEMPT)}
	if _, err := s.Control(ctx, attempt); status.Code(err) != codes.FailedPrecondition {
		t.Fatalf("attempt before initialize: %v", err)
	}
	initialize := &wire.Packet{Version: 1, Generation: 9, Invocation: 5, Phase: uint32(wire.Phase_PHASE_INITIALIZE)}
	if _, err := s.Control(ctx, initialize); err != nil {
		t.Fatal(err)
	}
	finished := make(chan error, 1)
	go func() { _, err := s.Control(ctx, attempt); finished <- err }()
	<-started
	finalize := &wire.Packet{Version: 1, Generation: 9, Invocation: 5, Phase: uint32(wire.Phase_PHASE_FINALIZE)}
	if _, err := s.Control(ctx, finalize); status.Code(err) != codes.FailedPrecondition {
		t.Fatalf("finalize before settlement: %v", err)
	}
	cancel := &wire.Packet{Version: 1, Generation: 9, Invocation: 5, Phase: uint32(wire.Phase_PHASE_CANCEL)}
	if _, err := s.Control(ctx, cancel); err != nil {
		t.Fatal(err)
	}
	if err := <-finished; status.Code(err) != codes.Canceled {
		t.Fatalf("attempt did not settle with cancellation: %v", err)
	}
	if _, err := s.Control(ctx, finalize); err != nil {
		t.Fatal(err)
	}
	if _, err := s.Control(ctx, attempt); status.Code(err) != codes.NotFound {
		t.Fatalf("finalized invocation remained callable: %v", err)
	}
}

func TestLifecycleAdmission(t *testing.T) {
	s := &server{operation: Operation{Owner: "test", Name: "greet"}, token: "secret", generation: 9}
	ctx := metadata.NewIncomingContext(context.Background(), metadata.Pairs("authorization", s.token))
	prepare := &wire.Packet{
		Version: 1, Generation: 9, Phase: uint32(wire.Phase_PHASE_PREPARE), Invocation: 5,
		Operation: "test.greet", Fingerprint: wire.SchemaFingerprint[:], Context: make([]byte, 32),
	}
	if _, err := s.Control(ctx, prepare); status.Code(err) != codes.FailedPrecondition {
		t.Fatalf("operation admitted before lifecycle initialization: %v", err)
	}
	valid := &wire.Initialization{ConfigJson: []byte("null"), GenerationContext: make([]byte, 32)}
	for _, test := range []struct {
		name    string
		request *wire.Initialization
		code    codes.Code
	}{
		{"malformed JSON", &wire.Initialization{ConfigJson: []byte("{"), GenerationContext: make([]byte, 32)}, codes.InvalidArgument},
		{"missing context", &wire.Initialization{ConfigJson: []byte("null")}, codes.InvalidArgument},
		{"unsupported callback", &wire.Initialization{ConfigJson: []byte("null"), GenerationContext: make([]byte, 32), Assignments: []*wire.Assignment{{Operation: "host.callback"}}}, codes.InvalidArgument},
		{"valid", valid, codes.OK},
	} {
		t.Run(test.name, func(t *testing.T) {
			payload, err := proto.Marshal(test.request)
			if err != nil {
				t.Fatal(err)
			}
			_, err = s.attempt(&invocation{operation: "test.harmos.initialize", payload: payload})
			if status.Code(err) != test.code {
				t.Fatalf("initialization code = %v, want %v", status.Code(err), test.code)
			}
		})
	}
	if _, err := s.Control(ctx, prepare); err != nil {
		t.Fatal(err)
	}
	s.invocation.cancel()
}

func TestBootstrapRefusesInvalidCredentials(t *testing.T) {
	operation := Operation{Owner: "test", Name: "greet", Run: func(context.Context, []byte) ([]byte, error) { return nil, nil }}
	settings := Settings{MessageBytes: 1024, ConcurrentRPCs: 2, StreamWindowBytes: 65536, ConnectionWindowBytes: 65536}
	for _, bootstrap := range []string{
		`[]`, `["not-hex",1]`, `["00",1]`, `["` + strings.Repeat("00", 32) + `",0]`,
	} {
		if err := Serve(strings.NewReader(bootstrap), io.Discard, operation, settings); err == nil {
			t.Fatal("invalid bootstrap admitted")
		}
	}
}
