// SPDX-License-Identifier: Apache-2.0

using System.Linq;

if (args.FirstOrDefault() == "--install-itself") { await InstallItself.Run(args[1..]); return; }
await new Harmos.Conformance.Fixture().Build().ServeAsync(remote: args.Contains("--remote"));
