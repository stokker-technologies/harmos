/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#pragma once

/* === Imports === */

// Standard Library Imports
#include <condition_variable>
#include <deque>
#include <mutex>
#include <thread>

// Third Party Imports
#include <grpcpp/grpcpp.h>

// Local Imports
#include "harmos/harmos.hpp"

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

namespace harmos::detail {
// Each Call Owns Its Handler and One Flow-Controlled Outgoing Message.
template<class I, class O> class Duplex : public grpc::ServerBidiReactor<I, O> {
public:
    Duplex() { this->StartSendInitialMetadata(); }
    I input;
    std::thread worker;
    std::recursive_mutex mutex;
    std::condition_variable_any room;
    std::deque<O> output;
    bool ending = false;
    bool finishing = false;
    bool metadata_sent = false;
    bool read_requested = false;
    grpc::Status status;
    std::function<void()> settled;

    void read_next() {
        std::lock_guard lock(mutex);
        if (ending) return;
        if (metadata_sent) this->StartRead(&input);
        else read_requested = true;
    }
    void OnSendInitialMetadataDone(bool ok) override {
        std::lock_guard lock(mutex);
        if (!ok) {
            closed();
            end({grpc::StatusCode::CANCELLED, "Transport Closed"});
            return;
        }
        metadata_sent = true;
        if (read_requested && !ending) this->StartRead(&input);
    }
    void write(O message) {
        std::unique_lock lock(mutex);
        room.wait(lock, [&] { return output.empty() || ending; });
        if (ending) throw TransportError(1, "Call Closed");
        output.push_back(std::move(message));
        this->StartWrite(&output.front());
    }
    void end(grpc::Status result) {
        std::lock_guard lock(mutex);
        if (ending) return;
        ending = true;
        status = std::move(result);
        room.notify_all();
        if (output.empty()) {
            finishing = true;
            this->Finish(status);
        }
    }
    void OnWriteDone(bool ok) override {
        std::lock_guard lock(mutex);
        output.pop_front();
        room.notify_all();
        if (!ok && !ending) {
            closed();
            end(grpc::Status(grpc::StatusCode::CANCELLED, "Transport Closed"));
        }
        if (ending && !finishing) {
            finishing = true;
            this->Finish(status);
        }
    }
    void OnCancel() override {
        closed();
        end(grpc::Status(grpc::StatusCode::CANCELLED, "Transport Closed"));
    }
    void OnDone() override {
        closed();
        if (worker.joinable()) worker.join();
        settle_children();
        auto signal = std::move(settled);
        delete this;
        if (signal) signal();
    }
    virtual void closed() = 0;
    virtual void settle_children() {}
};

}

/* === Templates === */

/* === Public Declarations === */
