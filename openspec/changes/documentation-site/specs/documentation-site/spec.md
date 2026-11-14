## ADDED Requirements

### Requirement: Documentation Archive per Publication

A stable release and a Nightly SHALL each publish one documentation archive holding the built
guides and the references for the operations and for the Rust, Python, C# and C++ SDKs, generated
from exactly the published source.

#### Scenario: Stable Release Carries Its Documentation

- **WHEN** a stable release is published
- **THEN** its assets include one documentation archive listed in the release index, and the archive holds the guides and all five references

#### Scenario: Stable Release Refuses Missing Documentation

- **WHEN** the documentation of a release tag fails to build
- **THEN** the release is not published

#### Scenario: Nightly Publishes Without Documentation

- **WHEN** the Nightly documentation job fails
- **THEN** the Nightly is still published and its report states that the documentation archive is missing

### Requirement: One Site Image with Every Supported Release

The site image SHALL contain one entry per supported release, a `latest` entry and a `dev` entry,
assembled from published documentation archives without building any earlier release.

#### Scenario: Reader Selects a Release

- **WHEN** a reader opens the selector
- **THEN** it offers `latest`, each supported release and `dev`, newest first

#### Scenario: Latest Is the Newest Stable Release

- **WHEN** a newer stable release is published and the image is assembled
- **THEN** `latest` shows the pages of that release, and a Nightly or prerelease never becomes `latest`

#### Scenario: Development Entry Follows the Branch

- **WHEN** the image is assembled after a Nightly
- **THEN** the `dev` entry shows the pages of that Nightly's documentation archive

### Requirement: Repository Holds Current Documentation Only

Documentation of earlier releases SHALL come from their published archives; the repository SHALL
hold no copy of an earlier release's guides.

#### Scenario: No Frozen Copies

- **WHEN** a release is cut
- **THEN** no folder of that release's guides is added to the repository

### Requirement: Installers Exclude Documentation

Installers SHALL NOT bundle the documentation archive.

#### Scenario: Installer Content

- **WHEN** an installer is packaged
- **THEN** it contains no documentation archive and no built documentation pages
