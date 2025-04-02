Concurrent Language
===================
# Setup/Install
This project requires installing Python, GCC, Java and Rust.
This project was developed with
- `Python=3.12`
- `GCC=12.2`
- `Java=21.0`
- `Rust=1.87`

Rust dependencies are managed automatically by Cargo.
## Python Dependencies
```bash
pip install -r requirements.txt
```
## C++ Dependencies
```bash
sudo apt-get install -y build-essential libgtest-dev librange-v3-dev
```
# Build and Run
## Compile
To compile a program:
```bash
make build FILE=$filename
```
This will compile the main executable to `bin/backend/main`.
Then
```bash
sudo ./backend/bin/main [ARGS]...
```
`sudo` is required to set the priorities and avoid interruption (you can still interrupt on a non-real-time kernel).
## Run
Alternatively, compile and run in one step:
```bash
make run FILE=$filename INPUT="$input"
```
## Running without `sudo`
It is possible to build/run without `sudo`.
To do this, set `USER_FLAG=1` as a Makefile argument.
# Test
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
# Benchmarking
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
The Python script was also slightly modified to include the extra time to set up multiple cores._
# Scripts
The main script is `./scripts/benchmark_visualization.py`.
All other scripts are used to generate the code size coefficients.
## Benchmark Visualization
`./scripts/benchmark_visualization.py` allows comparing the outputs of multiple runs.
It takes in a list of directories and options for the output.
The `-w` flag opens the resulting plot in the browser.
The `-o` flag accepts a pdf and writes a pdf to that file location.
For example, the following command compares the Python benchmark to the language benchmark, opening the result in a web browser and saving a plot to `plot.pdf`:
```bash
python benchmark_visualization.py logs/$python_benchmark logs/$language_benchmark -w -o plot.pdf
```
## Estimating Timing Coefficients
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
This will generate a new file `clean_vector.tsv` in the same folder.
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
Some of the coefficients should be zero but are assigned a positive value, and these should be deleted.
For example, `element_access` (tuple access) is usually free as the compiler inlines it.
However, this coefficient will often be positive and may need manually setting to zero.

# Repository Overview
## Frontend
The frontend converts text into C++ code to interact with the backend.
An overview of sections is:
- `Grammar.g4`
- `/parsing`
- `/type-checker`
- `/lowering`
- `/optimization`
- `/translation`
- `/emission`

Throughout the process, I use a pattern where enum fields have the same name as the type.
The `./from_variants` crate defines the directive `FromVariants` so that the types can be converted into the enum with `.into()`.
### Pipeline
`./pipeline` contains the orchestration code for the full compiler.
It performs argument parsing, then runs all the stages, displaying any errors that occur during type-checking.
### Grammar
- `Grammar.g4` specifies an ANTLR grammar with specifications for tokens and a parse tree.
It also contains comments with potential language extensions.
### Parsing
- `./parsing/grammar` is generated from the ANTLR grammar (via the Makefile), and contains Python code to lex and parse the text.
- `./parsing/operators.py` contains operators with specified precedences and associativity, as well as utilities for handling this.
- `./parsing/ast_nodes.py` contains the nodes for the AST and code to serialize them into JSON.
- `./parsing/parser.py` visits the parse tree and converts it into an AST.
- `./parsing/__main__.py` orchestrates the process by generating the parse tree with the ANTLR library, using the visitor to generate an AST, then serializing the result into JSON.
### Type Checking
The type-checker receives AST nodes in the form of JSON from the parsing stage.
- `./type-checker/src/ast_nodes.rs` contains equivalent nodes to `./parsing/ast_nodes.py` for deserializing.
- `./type-checker/src/prefix.rs` contains the program prefix with definitions of `&&` and `||` (done natively by the language).
- `./type-checker/src/utils.rs` contains a utility for detecting duplicates in parametric lists.
- `./type-checker/src/type_check_nodes.rs` contains definitions of annotated AST nodes that will be generated after the type-checking process.
- `./type-checker/src/type_checker.rs` contains the `TypeChecker` to type check a program and generate a `TypedProgram` or `TypeCheckError`.
### Lowering
Lowering converts the annotated AST into an intermediate representation.
- `./lowering/src/intermediate_nodes.rs` contains definitions for the intermediate representation.
- `./lowering/src/copy_propagation.rs` defines an `CopyPropagator` to remove registers that only alias another value.
- `./lowering/src/lower.rs` defines the `Lowerer` to convert the program from an annotated AST into the intermediate representation.
- `./lowering/src/expression_equality_checker.rs` defines an `ExpressionEqualityChecker` to determine if two expressions are equivalent when testing.
The intermediate representation gives each variable a unique id so this ensures that two expressions using different sets of unique ids are the same.
- `./lowering/src/type_equality_checker.rs` defines a `TypeEqualityChecker` to determine if two types are equivalent.
This is useful when handling type-aliases or recursive types.
- `./lowering/src/fn_inst.rs` contains utilities for identifying the lambda associated with a function call.
- `./lowering/src/recursive_fn_finder.rs` defines a `RecursiveFnFinder`, which identifies functions that might contain recursive calls.
### Optimization
- `./optimization/src/refresher.rs` define a `Refresher` to update functions that have duplicated variables or need variables from a new set for an optimization.
- `./optimization/src/dead_code_analysis.rs` contains a `DeadCodeAnalyzer` to remove dead code, including unused variables, arguments and functions.
- `./optimization/src/equivalent_expression_elimination.rs` contains an `EquivalentExpressionOptimizer` to remove duplicated expressions.
- `./optimization/src/inlining.rs` contains an `Inliner` to inline function calls.
- `./optimization/src/optimizer.rs` runs the optimizations based on the command-line arguments.
### Translation
The translation stage bridges between the intermediate representation and C++ code.
The outputs from this stage are machine nodes, which contain all the information to quickly generate C++ code.
- `./translation/src/machine_nodes.rs` defines machine nodes that mirror the C++ code.
- `./translation/src/named_vector.rs` defines a `define_named_vector` macro to generate vectors with named fields that can be added.
- `./translation/src/code_vector.rs` uses this macro to define a `CodeVector` and then calculate code vectors for a program.
- `./translation/src/code_size.rs` defines a `CodeSizeEstimator` to generate approximate bounds on the size of a function definition.
- `./translation/src/weakener.rs` defines a `Weakener` to introduce weak pointers and allocators to manage recursive cycles in functions.
- `./translation/src/translator.rs` defines the `Translator` to convert from the intermediate representation into the machine nodes.
### Emission
The emission stage generates C++ code that can be compiled, linked and run.
- `./emission/src/type_formatter.rs` contains a `TypeFormatter` and a `TypesFormatter` to convert machine node types into C++ types.
- `./emission/src/emission.rs` contains the `Emitter` to convert machine nodes into C++ code.

## Backend
The backend is written as a header-only library with template definitions.
The heavy use of template-metaprogramming means that files are split into header declarations (`.hpp`) and template implementations (`.tpp`). In the list below, I only include the `.hpp` files, but the `.tpp` files are also included.
### Lazy Values
- `./backend/include/lazy/lazy.hpp` defines type-specific `Lazy<T>` for calculating values.
- `./backend/include/lazy/types.hpp` contains utilities for handling and manipulating types with lazy instances.
### Functions
- `./backend/include/fn/fn_inst.hpp` defines `FnInst` (function instance) and implementations for closures.
- `./backend/include/fn/fn_gen.hpp` defines `FnGen` (function generator) and implementations for closures.
- `./backend/include/fn/operators.hpp` defines built-in operators.
- `./backend/include/fn/types.hpp` contains type aliases for function types.
### Work
- `./backend/include/work/work.hpp` defines a work item and its specialization with a typed function.
- `./backend/include/work/runner.hpp` defines a `WorkRunner` to process work items.
- `./backend/include/work/status.hpp` defines the status transitions for a work runner.
- `./backend/include/work/work_request.hpp` defines a `WorkRequest` for communicating work between work runners.
### Types
- `./backend/include/types/compound.hpp` defines compound types for product and union types.
- `./backend/include/types/display.hpp` defines utilities for displaying the resulting value when the program is complete.
- `./backend/include/types/builtin.hpp` defines builtin types (`Int` and `Bool`).
### Utilities
- `./backend/include/data_structures/lock.hpp` defines a lock and related utility functions.
- `./backend/include/data_structures/atomic_shared_enum.hpp` defines `AtomicSharedEnum` for synchronizing multiple enums in a single atomic.
- `./backend/include/data_structures/cyclic_queue.hpp` defines `CyclicQueue` with thread-safe operations.
- `./backend/include/time/utils.hpp` contains utils for timing the program.
- `./backend/include/types/utils.hpp` defines type utilities.
- `./backend/include/lazy/fns.hpp` contains utilities for manipulating lazy values.
### Main Execution
- `./backend/include/system/work_manager.hpp` provides interfaces that are not specific to thread-runners.
- `./backend/include/system/thread_manager.hpp` contains utilities for managing OS threads at setup and tear-down.
- `./backend/include/main/include.hpp` contains a list of headers that can be included by the main file.
- `./backend/src/main.cpp` handles the main program by reading command line arguments, then timing the execution, and displaying the output.
- `./backend/include/main/main.hpp` is where the frontend writes the C++ for translation.
### Development
- `./backend/src/sleep.cpp` is a utility program to sleep for 10 seconds.
- `./backend/test/` defines tests for the backend.
- `./backend/include/main/user.hpp` is a placeholder for overriding operations that require sudo.
- `./backend/wrap/` overrides setting the affinity and priority so that different variants can be run at different priorities.

## Other Files
### Programs
- `./samples` contains sample programs.
- `./benchmark` contains programs for benchmarking and their inputs.
- `./timing/` contains code for estimating the coefficients of code vectors.
### Scripts
- The scripts are descripted in [__Scripts__](#Scripts).
- `./scripts/plots/` contains files for generating the plots in the dissertation.
### CICD
- `.pre-commit-config.yaml` has pre-commit hooks for automatic linting and formatting on commit.
- `.github/workflows` contains scripts for building and testing on the remote server.
### Building
- `Makefile` and `backend/Makefile` define build operations and other utility functions.
- `requirements*.txt` and `**/Cargo.toml` contain dependencies for Python and Rust builds.
