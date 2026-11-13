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

#include <limits>
#include <optional>
#include <stdexcept>
#include "harmos/v1/storage_settings.pb.h"


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Macros === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Templates === */

/* === Public Declarations === */

namespace harmos {
inline int transfer_message_bytes(const ::harmos::storage::v1::StorageTransferSettings& settings = {}, std::optional<int> ceiling = {}) {
    const auto framing = static_cast<std::uint64_t>(::harmos::storage::v1::TRANSFER_FRAMING_BYTES);
    if (settings.chunk_bytes() == 0 || settings.chunk_bytes() > static_cast<std::uint64_t>(std::numeric_limits<int>::max()) - framing)
        throw std::invalid_argument("Configure Transfer: StorageSettings.transfer.chunk_bytes exceeds transport capacity");
    const auto required = static_cast<int>(settings.chunk_bytes() + framing);
    if (ceiling && *ceiling < required) throw std::invalid_argument("Configure Transfer: message_bytes cannot carry StorageSettings.transfer.chunk_bytes plus framing");
    return ceiling.value_or(required);
}
}

