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
    static inline Locked<std::deque<WeakWorkT>> work_queue;
    static inline std::vector<Locked<std::deque<WorkT>>> private_work_stacks;
    static WorkT finish_work;
    static inline std::vector<std::atomic<unsigned>> counters;
    static void enqueue(WorkT work);
    static void priority_enqueue(WorkT work);
    static void try_priority_enqueue(WorkT work);
    template <typename Ret, typename... Args>
    static Ret run(TypedFn<Ret, Args...> fn, Args... args);
    template <typename... Vs> static void await(Vs &...vs);
    template <typename... Vs> static void await_all(Vs &...vs);

  protected:
    template <typename... Vs> static void await_restricted(Vs &...vs);
    static std::monostate main(std::atomic<WorkT> *ref);
    static std::pair<WorkT, bool> get_work();
    static bool break_on_work(std::pair<WorkT, bool> work, Continuation &c);
    static void exit_early(Continuation &c);
    template <typename... Vs> static bool all_done(Vs &&...vs);
};

struct stack_inversion : public std::exception {
    stack_inversion() = default;
    const char *what() const noexcept override { return "Stack inversion"; }
};
struct finished : public std::exception {};
