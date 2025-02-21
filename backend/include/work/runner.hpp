#pragma once

#include "data_structures/lock.hpp"
#include "fn/continuation.tpp"
#include "system/thread_manager.tpp"
#include "work/work.hpp"

#include <atomic>
#include <deque>
#include <exception>

struct WorkRunner {
    friend class WorkManager;
    explicit WorkRunner(const ThreadManager::ThreadId &id);
    explicit WorkRunner(const unsigned &id);

    ThreadManager::ThreadId id;
    static inline Locked<std::deque<WeakWorkT>> shared_work_queue;

  protected:
    static WorkT finish_work;
    std::atomic<unsigned> counter;
    Locked<std::deque<WorkT>> private_work_stack;

    void main(std::atomic<WorkT> *ref);

    std::pair<WorkT, bool> get_work();
    static void enqueue(WorkT work);
    void priority_enqueue(WorkT work);
    void try_priority_enqueue(WorkT work);
    bool break_on_work(std::pair<WorkT, bool> work, Continuation &c);
    void exit_early(Continuation &c);

    template <typename... Vs> void await_restricted(Vs &...vs);
    template <typename... Vs> bool all_done(Vs &&...vs);
    template <typename... Vs> void await(Vs &...vs);
    template <typename... Vs> void await_all(Vs &...vs);
    template <typename T> void await_variants(T &v);
    template <typename... Ts>
    void await_variants(std::shared_ptr<Lazy<VariantT<Ts...>>> &l);
};

struct stack_inversion : public std::exception {
    stack_inversion() = default;
    const char *what() const noexcept override { return "Stack inversion"; }
};
struct finished : public std::exception {};
