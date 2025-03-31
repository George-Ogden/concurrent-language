# Concurrent Language
## Setup/Install
This project requires installing Python, GCC, Java and Rust.
This project was developed with
- `Python=3.12`
- `GCC=12.2`
- `Java=21.0`
- `Rust=1.87`

Rust dependencies are managed automatically by Cargo.
### Python Dependencies
```bash
pip install -r requirements.txt
```
### C++ Dependencies
```bash
sudo apt-get install -y build-essential libgtest-dev librange-v3-dev
```
## Build and Run
### Compile
To compile a program,
```bash
make build FILE=$filename
```
This will compile the main executable to `bin/backend/main`.
Then
```bash
sudo ./backend/bin/main [ARGS]...
```
`sudo` is required to set the priorities and avoid interruption (you can still interrupt on a non real-time kernel).
### Run
Alternatively, compile and run in one step.
```bash
make run FILE=$filename INPUT="$input"
```
### Running without `sudo`
It is possible to build/run without `sudo`.
To do this, set `USER_FLAG=1` as a Makefile argument.
## Test
To run Python tests, install the development dependencies.
```bash
pip install -r requirements-dev.txt`
```
To run all tests:
```bash
make test
```
This will also test the sample programs.
To run just the backend tests:
```bash
make -C backend test
```
To run just Python tests:
```bash
pytest .
```
To run Rust tests for a specific directory:
```bash
cargo test --manifest-path $directory/Cargo.toml
```
## Benchmarking
Benchmarking requires access to `sudo`.
You will be prompted to enter the password when the first program is run.
To benchmark programs:
```bash
make benchmark
```
To run the Python benchmark:
```bash
make python_benchmark
```
Both benchmarks will create directories with the following structure:
```
.
├── logs
│   ├── YYYYMMDDHHMMSSNNNNNNNNN
│   │   ├── title.txt # empty file (or file containing "python benchmark" if the Python benchmark is run)
│   │   ├── git # parent commit hash
│   │   └── log.tsv # log with timing information
```
`log.tsv` contains the headers `name`, `args` and `duration` where `duration` is the runtime in nanoseconds.

_The benchmarking scripts were modified for the multi-core benchmarking to ensure the correct CPUs were used.
The Python script was also slightly modified to include the extra time to setup multiple cores._
## Scripts
The main script is `./scripts/benchmark_visualization.py`.
All other scripts are used for generating the code size coefficients.
### Benchmark Visualization
`./scripts/benchmark_visualization.py` allows comparing the outputs of multiple runs.
It takes in a list of directories and options for the output.
The `-w` flag opens the resulting plot in the browser.
The `-o` flag accepts a pdf and writes a pdf to that file location.
For example, the following command compares the Python benchmark to the language benchmark, opening the result in a web browser and saving a plot to `plot.pdf`:
```bash
python benchmark_visualization.py logs/$python_benchmark logs/$language_benchmark -w -o plot.pdf
```
### Estimating Timing Coefficients
The timing programs are stored in `./timings`.
To generate runtimes and vectors, run `make timings` (it is __strongly__ recommended to disable optimization in `./backend/Makefile` when doing this).
This will produce a directory with the following structure:
```
.
├── logs
│   ├── YYYYMMDDHHMMSSNNNNNNNNN
│   │   ├── vector.tsv # times and coefficients for timing programs
│   │   ├── git # parent commit hash
│   │   └── log.tsv # log with timing information
```
You can preview the timings and programs with:
```bash
python scripts/timings_visualization.py logs/$timing_folder/vector.tsv
```
You will notice that the original timings file has a lot of noise.
Therefore, remove the outliers by running:
```bash
python scripts/clean_outliers.py logs/$timing_folder
```
This will generate a new file `clean_vector.tsv` in the same file.
You can preview this folder with:
```bash
python scripts/timings_visualization.py logs/$timing_folder/clean_vector.tsv
```
and you should notice that the outliers have been removed.
You can experiment with the `-z` flag (standard deviation threshold) to modify how extreme the outliers are to be removed.
With the cleaned-up file, you can fit a model, which generates coefficients in JSON format.
You can fit, display and save the coefficients as follows:
```bash
python scripts/fit logs/$timing_folder/clean_vector.tsv | tee logs/$timing_folder/coefficients.json
```
Finally, display the resulting model:
```bash
python scripts/timings_visualization.py logs/$timing_folder/clean_vector.tsv logs/$timing_folder/coefficients.json
```
This will display the predicted times, as well as the recorded times for each model.

Getting good predictions takes some playing around.
Some of the coefficients should be zero but are assigned a positive value and these should be deleted.
For example, `element_access` (tuple access) is usually free as the compiler inlines it.
However, this coefficient will often be positive and may manually need setting to zero.
