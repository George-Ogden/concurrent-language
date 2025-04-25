#pragma once

#include "data_structures/cyclic_queue.hpp"
#include "data_structures/lock.hpp"
#include "system/thread_manager.tpp"
#include "work/finished.hpp"
#include "work/work.hpp"
#include "work/work_request.hpp"

#include <atomic>
#include <exception>
#include <functional>
#include <optional>
#include <vector>

struct WorkRunner {
    friend struct WorkManager;
    friend class RunnerTest;
    explicit WorkRunner(const ThreadManager::ThreadId &id);

    static inline unsigned num_cpus;
    ThreadManager::ThreadId id;
    static CyclicQueue<unsigned> work_request_queue;
    static std::vector<std::unique_ptr<WorkRequest>> work_requests;

    // Setup everything required to run a function.
    static void setup(unsigned num_cpus);

  protected:
    std::vector<WorkT> extra_works, small_works, large_works;

    void main(std::atomic<WorkT> *ref);
    /// Active wait, performing any other tasks and exiting if predicate is
    /// true.
    bool active_wait(std::function<bool()> predicate);
    /// Determine whether there are any requests from other workers.
    bool any_requests() const;
    /// Respond to a request with work, returning true if the request succeeds.
    bool respond(WorkT &work) const;

    /// Wait for exactly the values given as arguments.
    template <typename... Vs> void await_restricted(Vs &...vs);
    /// Determine if all the given values are done.
    template <typename... Vs> bool all_done(Vs &&...vs);
    /// Wait for top level values to finish (no recursion).
    template <typename... Vs> auto await(Vs &...vs);
    /// Wait for all values to finish (recursively).
    template <typename... Vs> void await_all(Vs &...vs);
    /// Wait for a specific variant.
    template <typename T> void await_variants(T &v);
    /// Wait for multiple variants.
    template <typename... Ts>
    void await_variants(std::shared_ptr<Lazy<VariantT<Ts...>>> &l);
};

struct stack_inversion : public std::exception {
    stack_inversion() = default;
    const char *what() const noexcept override { return "Stack inversion"; }
};
