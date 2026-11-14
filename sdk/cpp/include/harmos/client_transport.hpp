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
#include <functional>
#include <iomanip>
#include <memory>
#include <optional>
#include <random>
#include <sstream>
#include <string>
#include <utility>

// Third Party Imports
#include <grpcpp/grpcpp.h>

// Local Imports
#include "harmos/transfer.hpp"

// Third Party Imports

// Local Imports

/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Declarations === */

namespace harmos {
template <class T> class ClientStream {
public:
  ClientStream(std::shared_ptr<grpc::ClientContext> context, std::function<std::optional<T>()> next)
      : context_(std::move(context)), next_(std::move(next)) {}
  ClientStream(const ClientStream &) = delete;
  ClientStream &operator=(const ClientStream &) = delete;
  ClientStream(ClientStream &&) = default;
  ClientStream &operator=(ClientStream &&) = delete;
  ~ClientStream() {
    if (context_) {
      context_->TryCancel();
    }
  }
  std::optional<T> next() {
    if (finished_) {
      return {};
    }
    try {
      auto value = next_();
      finished_ = !value.has_value();
      return value;
    } catch (...) {
      finished_ = true;
      throw;
    }
  }
  void cancel() {
    context_->TryCancel();
  }

private:
  std::shared_ptr<grpc::ClientContext> context_;
  std::function<std::optional<T>()> next_;
  bool finished_ = false;
};
namespace detail {
inline std::string request_id() {
  std::random_device entropy;
  std::ostringstream identity;
  identity << std::hex << std::setfill('0');
  for (int index = 0; index < 4; ++index) {
    identity << std::setw(8) << entropy();
  }
  return identity.str();
}
} // namespace detail
} // namespace harmos
