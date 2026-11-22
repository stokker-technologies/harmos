/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod stream_consumer;

/* === Public Functions === */

fn main() -> harmos::Result<()> {
    stream_consumer::plugin().run()
}
