#pragma once

#include "fn/types.hpp"
#include "work/runner.hpp"
#include "work/work.hpp"

#include <atomic>
#include <memory>
#include <utility>
#include <vector>

struct WorkManager {
    template <typename Ret, typename... Args>
    static LazyT<Ret> run(FnT<Ret, Args...> fn, Args... args);
    static std::monostate main(std::atomic<WorkT> *ref);
    template <typename... Vs> static auto await(Vs &...vs);
    template <typename... Vs> static void await_all(Vs &...vs);
    static inline std::vector<std::unique_ptr<WorkRunner>> runners;
};
