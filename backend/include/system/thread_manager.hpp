#pragma once

#include <range/v3/algorithm/transform.hpp>
#include <range/v3/numeric/iota.hpp>

#include <algorithm>
#include <atomic>
#include <cstring>
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

    static unsigned int hardware_concurrency() {
        return std::thread::hardware_concurrency();
    }

    static unsigned int available_concurrency() {
        return num_cpus_override.value_or(hardware_concurrency());
    }
    static void override_concurrency(unsigned int num_cpus) {
        num_cpus_override = num_cpus;
        set_shared_affinity();
    }
    static void reset_concurrency_override() {
        num_cpus_override = std::nullopt;
        set_shared_affinity();
    }

    static void register_self(ThreadId cpu_id) {
        id_conversion_table[std::this_thread::get_id()] = cpu_id;
    }

    static ThreadId get_id() {
        return id_conversion_table.at(std::this_thread::get_id());
    }

    static unsigned set_affinity(unsigned cpu_id) {
        cpu_set_t cpuset;
        CPU_ZERO(&cpuset);
        CPU_SET(cpu_id, &cpuset);

        int result =
            pthread_setaffinity_np(pthread_self(), sizeof(cpu_set_t), &cpuset);
        if (result != 0) {
            throw std::runtime_error("Failed to set CPU affinity: " +
                                     std::string(strerror(errno)));
        }
        return cpu_id;
    }
    static void set_shared_affinity() {
        cpu_set_t cpuset;
        CPU_ZERO(&cpuset);
        for (unsigned cpu_id = 0; cpu_id < available_concurrency(); cpu_id++) {
            CPU_SET(cpu_id, &cpuset);
        }
        int result =
            pthread_setaffinity_np(pthread_self(), sizeof(cpu_set_t), &cpuset);
        if (result != 0) {
            throw std::runtime_error("Failed to set CPU affinity: " +
                                     std::string(strerror(errno)));
        }
    }

    static int set_priority() {
        sched_param param;
        int policy = SCHED_FIFO;
        param.sched_priority = sched_get_priority_max(policy);

        if (pthread_setschedparam(pthread_self(), policy, &param) != 0) {
            throw std::runtime_error("Failed to set thread priority: " +
                                     std::string(strerror(errno)));
        }
        return param.sched_priority;
    }

    static inline std::atomic<int> waiting_threads;
    static void thread_setup(size_t cpu_id, bool verbose = false) {
        size_t cpu = set_affinity(cpu_id);
        int priority = set_priority();
        m.lock();
        if (verbose) {
            std::cout << "Running on CPU " << cpu << " with priority "
                      << priority << std::endl;
        }
        register_self(cpu_id);
        waiting_threads.fetch_add(-1, std::memory_order_relaxed);
        m.unlock();
        while (waiting_threads.load(std::memory_order_relaxed) > 0) {
        }
    }

    template <typename F, typename T>
    static auto thread_run(size_t cpu_id, F &&f, T &&arg,
                           bool verbose = false) {
        using R = std::invoke_result_t<F, T>;
        try {
            thread_setup(cpu_id, verbose);
            R result = f(std::forward<T>(arg));
            if constexpr (std::is_convertible_v<T, std::ostream &>) {
                if (verbose) {
                    m.lock();
                    std::cout << "Thread on CPU " << cpu_id
                              << " finished with result " << result
                              << std::endl;
                    m.unlock();
                }
            }
            return result;
        } catch (const std::exception &e) {
            m.lock();
            std::cerr << e.what() << std::endl;
            m.unlock();
            throw e;
        }
    }

    struct RunConfig {
        unsigned int num_cpus;
        bool verbose = false;
    };

    template <typename F, typename T>
    static void run_multithreaded(F thread_body, T arg,
                                  const RunConfig &run_config) {
        override_concurrency(run_config.num_cpus);
        std::vector<ThreadId> cpu_ids(run_config.num_cpus);
        ranges::iota(cpu_ids, 0);
        waiting_threads.store(run_config.num_cpus, std::memory_order_relaxed);

        std::vector<std::thread> threads;
        ranges::transform(
            cpu_ids, std::back_inserter(threads),
            [&thread_body, &arg, &run_config](auto cpu_id) {
                return std::thread([cpu_id, &thread_body, &arg, &run_config]() {
                    thread_run(cpu_id, thread_body, arg, run_config.verbose);
                });
            });

        for (auto &thread : threads) {
            if (thread.joinable()) {
                thread.join();
            }
        }
    }

  private:
    static inline std::mutex m;
    static inline std::optional<unsigned int> num_cpus_override;
    static inline std::map<std::thread::id, ThreadId> id_conversion_table;
};
