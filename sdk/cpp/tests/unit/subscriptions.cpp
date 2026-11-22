/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

// Standard Library Imports
#include <cstdint>
#include <map>
#include <memory>
#include <variant>
#include <vector>

// Third Party Imports
#include <catch2/catch_test_macros.hpp>

// Local Imports
#include "harmos/harmos.hpp"
#include "harmos/plugin/v1/subscriptions.pb.h"
#include "invocation.hpp"

using google::protobuf::Timestamp;
using harmos::plugin::Context;
using harmos::plugin::Gap;
using harmos::plugin::HostOperation;
using harmos::plugin::Plugin;
using harmos::plugin::Registrar;
using harmos::plugin::StreamRow;
using harmos::plugin::TimeGetResponse;
using harmos::plugin::detail::Invocation;
using harmos::plugin::v1::ChildResponse;
using harmos::plugin::v1::InstanceInvokeResponse;
using harmos::plugin::v1::SubscriptionsSubscribeResponse;

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Declarations === */

TEST_CASE("Typed Subscriptions Preserve Timestamps Gaps and Independent Closure") {
  Plugin plugin("fixture", "1.0.0");
  plugin.add([](Registrar &ctx) {
    ctx.add(HostOperation::SubscriptionsSubscribe);
  });
  const auto declarations = plugin.declarations();
  auto state = std::make_shared<Invocation>();
  for (const auto &requirement : declarations.requires_())
    state->requirements.emplace(requirement.id(), requirement.fingerprint());
  std::map<std::uint64_t, unsigned> positions;
  std::vector<std::uint64_t> cancelled;
  Timestamp source;
  source.set_seconds(7);
  source.set_nanos(123456789);
  Timestamp received;
  received.set_seconds(11);
  received.set_nanos(987654321);
  state->emit = [weak = std::weak_ptr<Invocation>(state), &positions, &cancelled, &source, &received](InstanceInvokeResponse frame) {
    auto owner = weak.lock();
    REQUIRE(owner);
    if (frame.has_child()) {
      REQUIRE(frame.child().watch());
      positions.emplace(frame.child().call(), 0);
      return;
    }
    if (frame.has_child_cancel()) {
      cancelled.push_back(frame.child_cancel().call());
      return;
    }
    REQUIRE(frame.has_child_demand());
    const auto call = frame.child_demand().call();
    SubscriptionsSubscribeResponse response;
    switch (positions.at(call)++) {
    case 0:
      response.set_opened(true);
      break;
    case 1: {
      TimeGetResponse value;
      *value.mutable_now() = source;
      auto row = response.mutable_row();
      row->set_payload(value.SerializeAsString());
      *row->mutable_source() = source;
      *row->mutable_received() = received;
      break;
    }
    default:
      response.set_missed(17);
    }
    ChildResponse reply;
    reply.set_call(call);
    reply.set_item(response.SerializeAsString());
    REQUIRE(owner->receive(reply));
  };
  Context context(state);
  auto first = context.subscribe<TimeGetResponse>("fixture.samples", 4096);
  const auto first_call = state->next;
  auto second = context.subscribe<TimeGetResponse>("fixture.samples", 4096);
  const auto second_call = state->next;
  auto item = first.next();
  REQUIRE(item);
  const auto &row = std::get<StreamRow<TimeGetResponse>>(*item);
  REQUIRE(row.value.now().SerializeAsString() == source.SerializeAsString());
  REQUIRE(row.source.SerializeAsString() == source.SerializeAsString());
  REQUIRE(row.received.SerializeAsString() == received.SerializeAsString());
  REQUIRE(std::get<Gap>(*first.next()).missed == 17);
  first.close();
  first.close();
  REQUIRE(cancelled.back() == first_call);
  REQUIRE_FALSE(state->watches.contains(first_call));
  REQUIRE(state->watches.contains(second_call));
  REQUIRE_FALSE(first.next());
  REQUIRE(std::holds_alternative<StreamRow<TimeGetResponse>>(*second.next()));
  harmos::plugin::detail::cancel_children(state, "Fixture Scope Ended");
  REQUIRE_THROWS_AS(second.next(), harmos::Failure);
  second.close();
}
