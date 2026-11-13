/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


/* === Imports === */

#include <iostream>
#include "harmos/harmos.hpp"
#include "harmos/client.hpp"
#include "harmos/conformance/v1/fixture.pb.h"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Functions === */

int install_itself(int argc, char** argv) {
    using Value = harmos::conformance::v1::Value;
    if (argc == 3 && std::string(argv[2]) == "serve") {
        harmos::Plugin("example", "1.0.0")
            .operation(harmos::Operation<Value, Value>("example.echo", harmos::wire::VISIBILITY_PUBLIC, [](harmos::Context&, Value request) { return request; }))
            .serve("127.0.0.1:0");
        return 0;
    }
    ::harmos::storage::v1::StorageTransferSettings transfer;
    auto client = harmos::Client::connect(argv[2], transfer);
    // Install and Start This Codebase's Running Instance
    harmos::v1::PluginsCreateRequest create;
    create.set_request_id("self-install"); create.set_name("example");
    create.mutable_placement()->mutable_remote()->set_url(argv[3]); create.mutable_value()->set_enabled(true);
    client.plugins.create(create);
    harmos::v1::PluginsWatchRequest selection; selection.set_name("example");
    auto watch = client.plugins.watch(selection);
    harmos::v1::PluginEntry ready;
    harmos::v1::PluginsWatchResponse event;
    while (watch.reader->Read(&event)) {
        if (event.has_snapshot()) {
            for (const auto& entry : event.snapshot().entries()) if (entry.state() == harmos::v1::PLUGIN_STATE_READY) ready = entry;
        } else if (event.has_change() && event.change().has_entry() && event.change().entry().state() == harmos::v1::PLUGIN_STATE_READY) {
            ready = event.change().entry();
        }
        if (ready.state() == harmos::v1::PLUGIN_STATE_READY) break;
    }
    if (ready.state() != harmos::v1::PLUGIN_STATE_READY) throw std::runtime_error("Watch Plugins: closed before Ready");
    // Invoke Its Public Operation Through the Host
    harmos::v1::InvocationsInvokeRequest invoke;
    for (const auto& contract : ready.description().catalog().operations()) if (contract.id() == "example.echo") {
        invoke.set_operation(contract.id()); invoke.set_fingerprint(contract.fingerprint());
    }
    std::string data(static_cast<std::size_t>(transfer.chunk_bytes()), '\0');
    data.replace(0, 5, "hello");
    Value request; request.set_value(data); invoke.set_payload(request.SerializeAsString());
    const auto reply = client.invocations.invoke(invoke);
    Value response;
    if (!response.ParseFromString(reply.payload())) throw std::runtime_error("Invoke example.echo: invalid response");
    if (response.value() != data) throw std::runtime_error("Invoke example.echo: payload changed");
    std::cout << "hello" << std::endl;
    harmos::v1::PluginsDeleteRequest remove; remove.set_name("example"); remove.set_expected_revision(ready.revision());
    client.plugins.delete_(remove);
    return 0;
}


/* === Private Functions === */

/* === Entry Point === */
