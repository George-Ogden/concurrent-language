#pragma once

#include "data_structures/cyclic_queue.hpp"
#include "data_structures/lock.hpp"
#include "system/thread_manager.tpp"
#include "work/work.hpp"
#include "work/work_request.hpp"

#include <atomic>
#include <exception>
#include <functional>
#include <optional>
#include <vector>

struct WorkRunner {
    friend class WorkManager;
    friend class RunnerTest;
    explicit WorkRunner(const ThreadManager::ThreadId &id);

    static inline unsigned num_cpus;
    ThreadManager::ThreadId id;
    static std::atomic<bool> done_flag;
    static CyclicQueue<unsigned> work_request_queue;
    static std::vector<std::unique_ptr<WorkRequest>> work_requests;
    static void setup(unsigned num_cpus);

  protected:
    std::atomic<unsigned> counter;

    void main(std::atomic<WorkT> *ref);
    bool active_wait(std::function<bool()> predicate);
    bool any_requests() const;
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
