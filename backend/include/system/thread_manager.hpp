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

class ThreadManager {
  public:
    using ThreadId = u_char;

    static unsigned int hardware_concurrency();
    static unsigned int available_concurrency();
    static void override_concurrency(unsigned int num_cpus);
    static void reset_concurrency_override();

    static void register_self(ThreadId cpu_id);
    static ThreadId get_id();
    static unsigned set_affinity(unsigned cpu_id);
    static void set_shared_affinity();
    static int set_priority();

    static void thread_setup(size_t cpu_id, bool verbose = false);

    template <typename F, typename T>
    static auto thread_run(size_t cpu_id, F &&f, T &&arg, bool verbose = false);

    struct RunConfig {
        unsigned int num_cpus;
        bool verbose = false;
    };

    template <typename F, typename T>
    static void run_multithreaded(F thread_body, T arg,
                                  const RunConfig &run_config);

  private:
    static inline std::mutex m;
    static inline std::atomic<int> waiting_threads;
    static inline std::optional<unsigned int> num_cpus_override;
    static inline std::map<std::thread::id, ThreadId> id_conversion_table;
};
