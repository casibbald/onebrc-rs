## Results using test data

```bash
❯ cat data/weather_stations.csv| wc -l
44693
```

```bash
❯ ./target/release/onebrc-rs data/weather_stations.csv # 
Total execution time: 67.096917ms
Total execution time: 74.611375ms
Total execution time: 78.083208ms

```

## Results using real data

See setting up and generating data at the bottom of the page.

### Results using real data with simple readlines implementation

The following results are from reading the entire 1br file into memory and processing.

```bash
❯ ./target/release/onebrc-rs --file=data/measurements.txt
Total execution time: 401.212742334s
Total execution time: 390.693394583s
Total execution time: 386.078361042s
```


### Results after adding coroutines and memory map, and using all CPU cores

Mac M1, 10 cores, 64GB RAM
```bash
Total execution time: 31.298131667s
Total execution time: 36.807488959s
Total execution time: 41.581745417s
```

Next optimization will need to be a better implementation of pipelining using coroutine and memory maps.

A threading + coroutine approach was attempted but the overhead of the threading was too high and the performance 
was worse than the simple readlines implementation.



## Setting up

Here are the steps and commands your friend can use to install Rust, Cargo, and set up the nightly toolchain on a Mac:

### Install Rust and Cargo

The easiest way to install Rust and Cargo is by using the rustup installer:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

	•	This will install rustup, which is the Rust toolchain installer, along with the latest stable versions of Rust and Cargo.
	•	Follow the on-screen instructions to complete the installation.

To ensure the path is properly set up, they may need to reload their shell or add this to their shell configuration file (~/.zshrc or ~/.bashrc):

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

After this, verify the installation:

```bash
rustc --version
cargo --version
```

### Install the Nightly Edition

To install the nightly Rust toolchain, use rustup:

```bash
rustup install nightly
```

To switch to nightly as the default toolchain globally:

```bash
rustup default nightly
```

Or, to use nightly only in a specific project directory:

```bash
rustup override set nightly
```

### Install Required Components for Nightly (Optional)

If additional components like rustfmt or clippy are needed, install them for nightly:

```bash
rustup component add rustfmt --toolchain nightly
rustup component add clippy --toolchain nightly
```

### Verify the Nightly Toolchain

Confirm that the nightly version is installed and being used:

```bash
rustc --version
```

This should display a version string containing nightly.

These commands should get your friend set up with Rust, Cargo, and the nightly edition on their Mac.

### Install Python


#### Mac
Assumes you already have homebrew installed. see https://brew.sh/ for installation instructions if you don't have it installed.

```bash
brew install python3 pip3
```

#### Debian Linux

It's likely your machine already has python3 installed but just incase.

```bash
sudo apt-get install python3 python3-pip
```

#### Windows

To install Python using Chocolatey, follow these steps:

#### First, ensure you have Chocolatey installed. If not, you can install it by running the following command in an elevated (Administrator) PowerShell:

 ```powershell
 Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = \
 [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex \
 ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
 ```

#### Once Chocolatey is installed, you can install Python by running:

 ```powershell
 choco install python make rustup.install
 rustup default nightly
 ```

#### Verify the installation by checking the Python version:

 ```powershell
 python --version
 ```

### Generating real data

Setting up all dependencies for generation tools.

```bash
cd data/
pip install -r hack/requirements.txt
```

Generating data in the data directory.

```bash
python hack/createMeasurements.py
```