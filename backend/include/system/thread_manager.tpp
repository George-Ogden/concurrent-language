#pragma once

#include "system/thread_manager.hpp"

#include <range/v3/algorithm/transform.hpp>
#include <range/v3/numeric/iota.hpp>

#include <atomic>
#include <cstring>
#include <iostream>
#include <mutex>
#include <optional>
#include <pthread.h>
#include <sched.h>
#include <stdexcept>
#include <thread>
#include <cstdlib>

std::mutex ThreadManager::m;
std::optional<unsigned int> ThreadManager::num_cpus_override;

unsigned ThreadManager::hardware_concurrency() {
    return std::thread::hardware_concurrency();
}

unsigned ThreadManager::available_concurrency() {
    const char * num_cpus_env_var = std::getenv("NUM_CPUS");
    if (num_cpus_env_var == nullptr){
        return num_cpus_override.value_or(hardware_concurrency());
    } else {
        return std::stoi(num_cpus_env_var);
    }
}

void ThreadManager::override_concurrency(unsigned num_cpus) {
    num_cpus_override = num_cpus;
    set_shared_affinity();
}

void ThreadManager::reset_concurrency_override() {
    num_cpus_override = std::nullopt;
    set_shared_affinity();
}

thread_local ThreadManager::ThreadId thread_id;
void ThreadManager::register_self(ThreadId cpu_id) {
    thread_id = cpu_id;
}

ThreadManager::ThreadId ThreadManager::get_id() {
    return thread_id;
}

unsigned ThreadManager::set_affinity(unsigned cpu_id) {
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

void ThreadManager::set_shared_affinity() {
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

int ThreadManager::set_priority() {
    sched_param param;
    int policy = SCHED_FIFO;
    param.sched_priority = sched_get_priority_max(policy);

    if (pthread_setschedparam(pthread_self(), policy, &param) != 0) {
        throw std::runtime_error("Failed to set thread priority: " +
                                 std::string(strerror(errno)));
    }
    return param.sched_priority;
}

void ThreadManager::thread_setup(size_t cpu_id, bool verbose) {
    size_t cpu = set_affinity(cpu_id);
    int priority = set_priority();
    if (verbose) {
        m.lock();
        std::cout << "Running on CPU " << cpu << " with priority " << priority
                  << std::endl;
        m.unlock();
    }
    register_self(cpu_id);
}

template <typename F, typename T>
auto ThreadManager::thread_run(size_t cpu_id, F &&f, T &&arg, bool verbose) {
    using R = std::invoke_result_t<F, T>;
    try {
        thread_setup(cpu_id, verbose);
        R result = f(std::forward<T>(arg));
        if constexpr (std::is_convertible_v<T, std::ostream &>) {
            if (verbose) {
                m.lock();
                std::cout << "Thread on CPU " << cpu_id
                          << " finished with result " << result << std::endl;
                m.unlock();
            }
        }
        return result;
    } catch (const std::exception &e) {
        m.lock();
        std::cerr << "Exception in thread " << cpu_id << ": " << e.what()
                  << std::endl;
        m.unlock();
        throw e;
    }
}

template <typename F, typename T>
void ThreadManager::run_multithreaded(F thread_body, T arg,
                                      const RunConfig &run_config) {
    override_concurrency(run_config.num_cpus);
    std::vector<ThreadId> cpu_ids(run_config.num_cpus);
    ranges::iota(cpu_ids, 0);

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
