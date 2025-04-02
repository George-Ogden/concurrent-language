#pragma once

#include <atomic>
#include <iostream>
#include <map>
#include <mutex>
#include <optional>
#include <pthread.h>
#include <sched.h>
#include <stdexcept>
#include <thread>
#include <vector>

/// Class for handling thread utilities, including setup and tear-down.
class ThreadManager {
  public:
    using ThreadId = u_char;

    /// Number of CPUs on the hardware.
    static unsigned int hardware_concurrency();
    /// Concurrency that has been defined as available.
    static unsigned int available_concurrency();
    /// Override available concurrency.
    static void override_concurrency(unsigned int num_cpus);
    /// Reset to hardware concurrency.
    static void reset_concurrency_override();

    /// Set the CPU id of a thread.
    static void register_self(ThreadId cpu_id);
    /// Get the CPU id of a thread.
    static ThreadId get_id();
    /// Bind the thread to a single CPU.
    static unsigned set_affinity(unsigned cpu_id);
    /// Set affinity for parent process (restricts stl algorithms).
    static void set_shared_affinity();
    /// Set priority to the max.
    static int set_priority();

    /// Setup stage that each thread runs before starting.
    static void thread_setup(size_t cpu_id, bool verbose = false);

    template <typename F, typename T>
    /// Run a function on a specified thread.
    static auto thread_run(size_t cpu_id, F &&f, T &&arg, bool verbose = false);

    struct RunConfig {
        unsigned int num_cpus;
        bool verbose = false;
    };

    /// Run a multi-threaded function no many CPUs, specified by the config.
    template <typename F, typename T>
    static void run_multithreaded(F thread_body, T arg,
                                  const RunConfig &run_config);

  private:
    static std::mutex m;
    static std::atomic<int> waiting_threads;
    static std::optional<unsigned> num_cpus_override;
};
