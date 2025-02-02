#pragma once

#include "data_structures/lock.hpp"
#include "system/thread_manager.hpp"

#include <atomic>
#include <deque>
#include <exception>
#include <utility>
#include <vector>

class Fn;

struct WorkManager {
    static inline Locked<std::deque<std::shared_ptr<Fn>>> queue;
    static inline std::vector<std::atomic<unsigned>> counters;
    static void call(std::shared_ptr<Fn> fn);
    static void run(std::shared_ptr<Fn> fn);
    template <typename... Vs> static void await(Vs &...vs);

  protected:
    template <typename... Vs> static void await_all(Vs &...vs);
    static std::monostate main(std::atomic<std::shared_ptr<Fn>> *ref);
    static std::shared_ptr<Fn> get_work();
    template <typename... Vs> static bool all_done(Vs &&...vs) {
        return (... && vs->done());
    }
};

struct stack_inversion : public std::exception {};
