#pragma once

#include "data_structures/cyclic_queue.hpp"
#include "data_structures/lock.hpp"
#include "system/thread_manager.tpp"
#include "work/work.hpp"

#include <atomic>
#include <deque>
#include <exception>
#include <optional>

struct WorkRunner {
    friend class WorkManager;
    friend class RunnerTest;
    explicit WorkRunner(const ThreadManager::ThreadId &id);

    static inline unsigned num_cpus;
    ThreadManager::ThreadId id;
    static std::atomic<bool> done_flag;
    static CyclicQueue<std::atomic<WorkT> *> work_request_queue;

  protected:
    std::atomic<unsigned> counter;

    void main(std::atomic<WorkT> *ref);
    void active_wait();
    bool any_requests() const;
    WorkT request_work() const;
    std::optional<std::atomic<WorkT> *> get_receiver() const;
    bool respond(WorkT &work) const;

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
