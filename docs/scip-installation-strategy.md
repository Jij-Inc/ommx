# SCIP Installation Strategy for OMMX

## Overview

This document outlines the strategy for installing and managing SCIP (Solving Constraint Integer Programs) across development, testing, and CI/CD environments.

## SCIP Version Information

- **Latest Stable**: SCIP Optimization Suite 9.2.4 (October 2024)
- **Beta**: SCIP 10.0.0 (in development)
- **Recommended for OMMX**: 9.2.4

## Installation Approaches

### 1. Local Development

#### Option A: Pre-built Debian Package (Recommended for Ubuntu)

```bash
# Download SCIP Optimization Suite 9.2.4 for Ubuntu 22.04
wget https://github.com/scipopt/scip/releases/download/v924/SCIPOptSuite-9.2.4-Linux-ubuntu22.deb

# Install
sudo dpkg -i SCIPOptSuite-9.2.4-Linux-ubuntu22.deb

# Install dependencies if needed
sudo apt-get install -f
```

**Pros:**
- Fast installation (~5 minutes)
- Pre-compiled and optimized
- Includes SoPlex and ZIMPL

**Cons:**
- Ubuntu version-specific
- Not available for all platforms

#### Option B: Build from Source

```bash
# Install dependencies
sudo apt-get update
sudo apt-get install -y \
    wget cmake g++ m4 xz-utils \
    libgmp-dev unzip zlib1g-dev \
    libboost-program-options-dev \
    libboost-serialization-dev \
    libboost-regex-dev \
    libboost-iostreams-dev \
    libtbb-dev libreadline-dev \
    pkg-config git liblapack-dev \
    libgsl-dev flex bison \
    libcliquer-dev gfortran

# Download and build
wget https://github.com/scipopt/scip/archive/refs/tags/v924.tar.gz
tar xzf v924.tar.gz
cd scip-924
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make -j$(nproc)
sudo make install
sudo ldconfig
```

**Pros:**
- Latest version always available
- Customizable build options
- Platform-independent

**Cons:**
- Slow build time (20-30 minutes)
- Requires build tools
- More complex

### 2. CI/CD (GitHub Actions)

#### Strategy: Cache-First with Fallback

```yaml
# Recommended approach: Cache pre-built binaries
steps:
  - name: Cache SCIP Installation
    id: cache-scip
    uses: actions/cache@v4
    with:
      path: |
        /usr/local/lib/libscip*
        /usr/local/include/scip/
        /usr/local/lib/cmake/scip/
      key: scip-9.2.4-${{ runner.os }}-${{ runner.arch }}

  - name: Install SCIP
    if: steps.cache-scip.outputs.cache-hit != 'true'
    run: |
      # Use pre-built .deb for Ubuntu runners
      wget -q https://github.com/scipopt/scip/releases/download/v924/SCIPOptSuite-9.2.4-Linux-ubuntu22.deb
      sudo dpkg -i SCIPOptSuite-9.2.4-Linux-ubuntu22.deb
```

**Benefits:**
- Fast CI runs (cached: <10s, uncached: ~5min)
- Reliable and repeatable
- Minimal impact on build times

### 3. Docker Containers

#### Official SCIP Container

```dockerfile
FROM scipoptsuite/scipoptsuite:9.2.4 AS scip-base

# Add OMMX dependencies
FROM rust:1.75 AS builder
COPY --from=scip-base /usr/local /usr/local

# Build OMMX with SCIP support
WORKDIR /app
COPY . .
RUN cargo build --release -p ommx-scip-adapter
```

#### Custom Multi-Stage Build

```dockerfile
FROM ubuntu:24.04 AS scip-builder

# Install SCIP
RUN apt-get update && apt-get install -y wget
RUN wget -q https://github.com/scipopt/scip/releases/download/v924/SCIPOptSuite-9.2.4-Linux-ubuntu22.deb \
    && dpkg -i SCIPOptSuite-9.2.4-Linux-ubuntu22.deb || true \
    && apt-get install -f -y

# Final image
FROM ubuntu:24.04
COPY --from=scip-builder /usr/local /usr/local
RUN ldconfig
```

## Platform Support Matrix

| Platform | Method | SCIP Version | Status |
|----------|--------|--------------|--------|
| Ubuntu 22.04 | .deb package | 9.2.4 | ✅ Recommended |
| Ubuntu 24.04 | .deb package | 9.2.4 | ⚠️ Compatible (22.04 pkg) |
| Ubuntu 20.04 | .deb package | 9.2.4 | ✅ Supported |
| macOS | Homebrew | Latest | ✅ `brew install scip` |
| Windows | Build from source | Any | ⚠️ Complex |
| Docker | Official image | 9.2.4 | ✅ Recommended |

## Installation Scripts

### Automated Detection Script

Create `scripts/install-scip.sh`:

```bash
#!/bin/bash
set -e

SCIP_VERSION="9.2.4"
CACHE_DIR="${HOME}/.cache/scip"

detect_platform() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if command -v lsb_release &> /dev/null; then
            DISTRO=$(lsb_release -is)
            VERSION=$(lsb_release -rs)
            echo "linux-${DISTRO,,}-${VERSION%%.*}"
        else
            echo "linux-unknown"
        fi
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    else
        echo "unsupported"
    fi
}

install_scip_ubuntu() {
    local version=$1
    local deb_url="https://github.com/scipopt/scip/releases/download/v${version//./}/SCIPOptSuite-${version}-Linux-ubuntu22.deb"

    echo "Installing SCIP ${version} for Ubuntu..."
    wget -q "$deb_url" -O /tmp/scip.deb
    sudo dpkg -i /tmp/scip.deb || sudo apt-get install -f -y
    rm /tmp/scip.deb
}

install_scip_macos() {
    echo "Installing SCIP via Homebrew..."
    brew install scip
}

install_scip_source() {
    local version=$1
    echo "Building SCIP ${version} from source..."

    # Install dependencies
    sudo apt-get update
    sudo apt-get install -y \
        cmake g++ libgmp-dev zlib1g-dev \
        libreadline-dev pkg-config

    # Download and build
    local tarball="https://github.com/scipopt/scip/archive/refs/tags/v${version//./}.tar.gz"
    wget -q "$tarball" -O /tmp/scip.tar.gz
    tar -xzf /tmp/scip.tar.gz -C /tmp
    cd "/tmp/scip-${version//./}"

    mkdir build && cd build
    cmake .. -DCMAKE_BUILD_TYPE=Release
    make -j$(nproc)
    sudo make install
    sudo ldconfig
}

main() {
    platform=$(detect_platform)

    case "$platform" in
        linux-ubuntu-*)
            install_scip_ubuntu "$SCIP_VERSION"
            ;;
        macos)
            install_scip_macos
            ;;
        *)
            echo "Unsupported platform: $platform"
            echo "Attempting source build..."
            install_scip_source "$SCIP_VERSION"
            ;;
    esac

    # Verify installation
    if pkg-config --exists scip; then
        echo "✓ SCIP installed successfully"
        pkg-config --modversion scip
    else
        echo "✗ SCIP installation failed"
        exit 1
    fi
}

main "$@"
```

## CI/CD Integration

### GitHub Actions Composite Action

Create `.github/actions/setup-scip/action.yml`:

```yaml
name: "Setup SCIP"
description: "Install SCIP Optimization Suite with caching"

inputs:
  scip-version:
    description: "SCIP version to install"
    required: false
    default: "9.2.4"
  cache-key-suffix:
    description: "Additional cache key suffix"
    required: false
    default: ""

runs:
  using: "composite"
  steps:
    - name: Cache SCIP
      id: cache-scip
      uses: actions/cache@v4
      with:
        path: |
          /usr/local/lib/libscip*
          /usr/local/include/scip/
          /usr/local/lib/cmake/scip/
          /usr/local/lib/pkgconfig/scip.pc
        key: scip-${{ inputs.scip-version }}-${{ runner.os }}-${{ runner.arch }}${{ inputs.cache-key-suffix }}
        restore-keys: |
          scip-${{ inputs.scip-version }}-${{ runner.os }}-

    - name: Install SCIP
      if: steps.cache-scip.outputs.cache-hit != 'true'
      shell: bash
      run: |
        SCIP_VERSION="${{ inputs.scip-version }}"
        SCIP_VERSION_NUM="${SCIP_VERSION//./}"

        if [[ "$RUNNER_OS" == "Linux" ]]; then
          # Ubuntu: use .deb package
          wget -q "https://github.com/scipopt/scip/releases/download/v${SCIP_VERSION_NUM}/SCIPOptSuite-${SCIP_VERSION}-Linux-ubuntu22.deb"
          sudo dpkg -i "SCIPOptSuite-${SCIP_VERSION}-Linux-ubuntu22.deb" || sudo apt-get install -f -y
        elif [[ "$RUNNER_OS" == "macOS" ]]; then
          brew install scip
        fi

    - name: Verify SCIP Installation
      shell: bash
      run: |
        if pkg-config --exists scip; then
          echo "✓ SCIP $(pkg-config --modversion scip) installed"
          pkg-config --cflags --libs scip
        else
          echo "✗ SCIP installation verification failed"
          exit 1
        fi
```

### Update Rust Workflow

```yaml
# .github/workflows/rust.yml
jobs:
  test-scip-adapter:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5

      - name: Setup Environment
        uses: ./.github/actions/setup-test-environment

      - name: Setup SCIP
        uses: ./.github/actions/setup-scip
        with:
          scip-version: "9.2.4"

      - name: Build SCIP Adapter
        run: cargo build -p ommx-scip-adapter

      - name: Test SCIP Adapter
        run: cargo test -p ommx-scip-adapter
```

## Development Workflow

### One-time Setup

```bash
# Clone repository
git clone https://github.com/Jij-Inc/ommx.git
cd ommx

# Install SCIP
./scripts/install-scip.sh

# Build OMMX with SCIP support
cargo build -p ommx-scip-adapter

# Run tests
cargo test -p ommx-scip-adapter
```

### Continuous Development

```bash
# Build
cargo build -p ommx-scip-adapter

# Test
cargo test -p ommx-scip-adapter

# Run example
cargo run --example knapsack -p ommx-scip-adapter
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_scip_available() {
        // Verify SCIP library is linked
        assert!(scip_ffi::SCIP_VERSION >= 900);
    }
}
```

### Integration Tests

```bash
# Test with actual SCIP installation
cargo test -p ommx-scip-adapter --test integration

# Test knapsack problem
cargo test -p ommx-scip-adapter test_knapsack
```

### CI/CD Test Matrix

```yaml
strategy:
  matrix:
    os: [ubuntu-22.04, ubuntu-24.04, macos-latest]
    scip-version: ["9.2.4"]
```

## Troubleshooting

### Common Issues

1. **Library not found**
   ```bash
   sudo ldconfig
   export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH
   ```

2. **pkg-config not finding SCIP**
   ```bash
   export PKG_CONFIG_PATH=/usr/local/lib/pkgconfig:$PKG_CONFIG_PATH
   ```

3. **Headers not found**
   ```bash
   export CPATH=/usr/local/include:$CPATH
   ```

## References

- [SCIP Official Installation Guide](https://scipopt.org/doc/html/INSTALL.php)
- [SCIP GitHub Releases](https://github.com/scipopt/scip/releases)
- [SCIP Optimization Suite 9.0](https://arxiv.org/abs/2402.17702)
- [Docker with SCIP](https://github.com/viktorsapozhok/docker-scip)

## Maintenance

### Version Updates

When updating SCIP version:
1. Update `SCIP_VERSION` in all scripts
2. Test locally
3. Update CI/CD cache keys
4. Update documentation
5. Test on all platforms

### Monitoring

- Track SCIP releases: https://github.com/scipopt/scip/releases
- Monitor breaking changes in SCIP C API
- Update bindgen configuration if needed
