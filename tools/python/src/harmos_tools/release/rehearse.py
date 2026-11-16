# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Own Release Rehearse."""

# === Imports === #

# Standard Library Imports
from pathlib import Path

import harmos_tools.release.assets
import harmos_tools.release.conformance

# Third Party Imports
# Local Imports
import harmos_tools.release.crates
import harmos_tools.release.python_registry
import harmos_tools.release.requirements
import harmos_tools.release.source

from . import layout, policy, version

# Type Checking Imports

# === Public Exports === #

# === Constants === #


# === Types === #

# === Classes === #

# === Public Functions === #


def dry(
    directory: Path,
    settings: dict,
    *,
    profile: str | None = None,
    require_all: bool = False,
    source_revision: str | None = None,
    release_notes: str | None = None,
    targets: list[str] | None = None,
    channel: str = "stable",
) -> None:
    """Stage The Active Profile's Packages Like A Release And Run The Native Downloads From Them, Offline."""
    tag = harmos_tools.release.source.release_tag(directory)
    version.run(directory, "check")
    # Release reviews package the release profile; every other run packages dev.
    profile = policy.profile("dev") if profile is None else profile
    root = harmos_tools.release.assets.staging_root(directory)
    targets = list(settings.get("targets", [])) if targets is None else targets
    present = [selected for selected in targets if (directory / "target/package" / profile / selected).is_dir()]
    missing = sorted(set(targets) - set(present))
    if missing and (require_all or harmos_tools.release.source.continuous_integration()):
        raise policy.ToolError(f"{profile} packages are missing for release targets: {', '.join(missing)}")
    if missing:
        print(f"note: no local {profile} packages for {', '.join(missing)}; CI stages every release target")
    harmos_tools.release.assets.stage(
        directory, settings, tag, present, profile=profile, source_revision=source_revision, release_notes=release_notes, channel=channel
    )
    files = layout.uploads(directory, staged=True)
    harmos_tools.release.assets.checksums(
        root / harmos_tools.release.assets.SHARED / harmos_tools.release.assets.SUMS, {item["name"]: item for item in files}
    )
    native = harmos_tools.release.source.native()
    if native in present:
        # The staged bytes themselves run, from an isolated directory, exactly as a user's download would.
        harmos_tools.release.conformance.verify(directory, settings, profile, native, root)
    print(
        f"staged {len(files) - 1} download(s) and {harmos_tools.release.assets.SUMS} for {', '.join(present) or 'no target'} in {root.relative_to(directory)}"
    )


def run(directory: Path, settings: dict, *, target: str | None = None, profile: str = "release", stage_only: bool = False) -> None:
    """Verify Selected Local Packages Or Every Hosted Target Without Publishing Credentials."""
    hosted = harmos_tools.release.source.continuous_integration()
    if profile not in {"dev", "release"}:
        raise policy.ToolError("rehearsal profile must be dev or release")
    if hosted and (profile != "release" or stage_only):
        raise policy.ToolError("hosted rehearsal requires complete release-profile verification")
    if hosted and target not in (None, "all"):
        raise policy.ToolError("hosted rehearsal requires all declared release targets")
    target = target or ("all" if hosted else "native")
    selected = harmos_tools.release.source.native_target(settings) if target == "native" else target
    if selected != "all" and selected not in settings.get("targets", []):
        raise policy.ToolError(f"undeclared release target: {selected}")
    targets = list(settings.get("targets", [])) if selected == "all" else [selected]
    credentials = policy.CredentialScope(settings, profile=profile)
    try:
        harmos_tools.release.requirements.refuse_overrides(directory)
        revision = harmos_tools.release.source.revision(directory) if stage_only else None
        dry(directory, settings, profile=profile, require_all=True, source_revision=revision, targets=targets)
        if not stage_only and harmos_tools.release.source.task_defined(directory, "release:publish:rust"):
            harmos_tools.release.crates.rehearse(directory, skip_published=False)
        if not stage_only:
            harmos_tools.release.python_registry.publish(directory, settings, dry_run=True)
    finally:
        credentials.close()
    print(f"release {'staging' if stage_only else 'rehearsal'} passed using staged {profile}-profile packages")


# === Private Functions === #

# === Entry Point === #
