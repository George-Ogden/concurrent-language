#pragma once

#include "fn/fn.hpp"
#include "work/runner.hpp"
#include "work/work.hpp"

#include <atomic>
#include <memory>
#include <utility>
#include <vector>

struct WorkManager {
    template <typename Ret, typename... Args>
    static Ret run(TypedFn<Ret, Args...> fn, Args... args);
    static std::monostate main(std::atomic<WorkT> *ref);
    static void enqueue(WorkT work);
    template <typename... Vs> static void await(Vs &...vs);
    template <typename... Vs> static void await_all(Vs &...vs);
    static inline std::vector<std::unique_ptr<WorkRunner>> runners;
};
