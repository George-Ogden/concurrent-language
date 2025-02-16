#pragma once

#include "data_structures/lock.tpp"
#include "fn/fn.hpp"
#include "system/thread_manager.hpp"
#include "work/work.hpp"

#include <atomic>
#include <deque>
#include <exception>
#include <utility>
#include <vector>

struct WorkManager {
    static inline Locked<std::deque<std::weak_ptr<Work>>> queue;
    static std::shared_ptr<Work> finish_work;
    static inline std::vector<std::atomic<unsigned>> counters;
    static void enqueue(std::shared_ptr<Work> work);
    template <typename Ret, typename... Args>
    static Ret run(TypedFn<Ret, Args...> fn, Args... args);
    template <typename... Vs> static void await(Vs &...vs);
    template <typename... Vs> static void await_all(Vs &...vs);

  protected:
    template <typename... Vs> static void await_restricted(Vs &...vs);
    static std::monostate main(std::atomic<std::shared_ptr<Work>> *ref);
    static std::shared_ptr<Work> get_work();
    template <typename... Vs> static bool all_done(Vs &&...vs);
};

struct stack_inversion : public std::exception {
    stack_inversion() = default;
    const char *what() const noexcept override { return "Stack inversion"; }
};
struct finished : public std::exception {};

struct FinishWork : public Work {
    void run() override;
};
