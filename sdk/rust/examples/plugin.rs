/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serve One Plugin Operation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::prelude::*;
use harmos::v1::{SettingDeclaration, SettingValue, SettingsDeclarations, setting_value};
use harmos_protocol::plugin::host::SettingsListRequest;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
#[harmos(package = "example.plugin.v1", file = "example/plugin/v1/echo.proto")]
struct EchoRequest {
    text: String,
}

#[derive(Message)]
#[harmos(package = "example.plugin.v1", file = "example/plugin/v1/echo.proto")]
struct EchoResponse {
    text: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

fn main() -> Result<()> {
    let settings = SettingsDeclarations {
        values: vec![SettingDeclaration {
            name: "prefix".into(),
            title: "Prefix".into(),
            description: "Text before each answer".into(),
            default: Some(SettingValue {
                value: Some(setting_value::Value::Text("echo: ".into())),
            }),
        }],
    };
    Plugin::new("echo", "1.0.0")
        .settings(settings)
        .requires::<SettingsListRequest>()
        .operation(Operation::new("example.plugin.v1.Echo/Call", echo).visibility(Visibility::Public))
        .serve()
}

/* === Private Functions === */

async fn echo(context: &Context, request: EchoRequest) -> Result<EchoResponse> {
    let settings = context.settings().await?.value.ok_or_else(|| Error::Invalid("Echo: Settings Missing".into()))?;
    let prefix = match settings.values.get("prefix").and_then(|setting| setting.value.as_ref()) {
        Some(setting_value::Value::Text(prefix)) => prefix,
        _ => return Err(Error::Invalid("Echo: Text Prefix Required".into())),
    };
    Ok(EchoResponse {
        text: format!("{prefix}{}", request.text),
    })
}

/* === Tests === */
