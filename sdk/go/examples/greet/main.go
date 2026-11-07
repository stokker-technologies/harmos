// SPDX-License-Identifier: Apache-2.0

package main

import (
	"context"
	"fmt"
	"os"

	sidecar "github.com/stokker-technologies/harmos/sdk/go"
	"github.com/stokker-technologies/harmos/sdk/go/wire"
	"google.golang.org/protobuf/proto"
)

func main() {
	// Declare One Operation Using The Generated Contract.
	operation := sidecar.Operation{Owner: "go_tools", Name: "greet", Run: greet}

	// The Example Owns Its Transport Budgets.
	settings := sidecar.Settings{
		MessageBytes: 2 * 1024 * 1024, ConcurrentRPCs: 16,
		StreamWindowBytes: 64 * 1024, ConnectionWindowBytes: 2 * 1024 * 1024,
	}

	// Bootstrap, Authenticate And Serve Until The Host Retires This Generation.
	if err := sidecar.Serve(os.Stdin, os.Stdout, operation, settings); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func greet(ctx context.Context, payload []byte) ([]byte, error) {
	// Decode, Answer And Encode With Ordinary Protobuf.
	var request wire.GreetRequest
	if err := proto.Unmarshal(payload, &request); err != nil {
		return nil, err
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	return proto.Marshal(&wire.GreetResponse{Greeting: "Hello, " + request.Name + "!"})
}
