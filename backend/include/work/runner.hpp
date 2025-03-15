#pragma once

#include "data_structures/lock.hpp"
#include "fn/continuation.tpp"
#include "system/thread_manager.tpp"
#include "work/work.hpp"

#include <atomic>
#include <deque>
#include <exception>

class StackSeparation;
struct WorkRunner {
    friend class WorkManager;
    friend class StackSeparation;
    explicit WorkRunner(const ThreadManager::ThreadId &id);

    static inline unsigned num_cpus;
    ThreadManager::ThreadId id;
    static inline Locked<std::deque<WeakWorkT>> shared_work_queue;
    static inline std::atomic<bool> done_flag;

  protected:
    std::atomic<unsigned> counter;
    Locked<std::deque<WorkT>> private_work_stack;
    WorkT current_work;

    void main(std::atomic<WorkT> *ref);

    WorkT get_work();
    void enqueue(WorkT work);
    bool break_on_work(WorkT &work, Continuation &c);

    template <typename... Vs> void await_restricted(Vs &...vs);
    template <typename... Vs> bool all_done(Vs &&...vs);
    template <typename... Vs> auto await(Vs &...vs);
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
