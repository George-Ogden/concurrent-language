#pragma once

#include "fn/types.hpp"
#include "work/runner.hpp"
#include "work/work.hpp"

#include <atomic>
#include <memory>
#include <utility>
#include <vector>

/// Class for handling shared work utilities.
struct WorkManager {
    template <typename Ret, typename... Args>
    // Run a function on multiple CPUs.
    static LazyT<Ret> run(FnT<Ret, Args...> fn, Args... args);
    // Main function for threads to execute.
    static std::monostate main(std::atomic<WorkT> *ref);
    // Enqueue work to be executed in the future.
    template <typename T> static void enqueue(const T &values);
    static void enqueue(const WorkT &work);
    // Wait for values to be computed.
    template <typename... Vs> static auto await(Vs &...vs);
    // Recursively wait for all values to be computed (useful for recursive
    // types or nested tuples).
    template <typename... Vs> static void await_all(Vs &...vs);
    static inline std::vector<std::unique_ptr<WorkRunner>> runners;
};
