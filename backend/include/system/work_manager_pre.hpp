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
    static inline Locked<std::deque<Fn *>> queue;
    static inline std::vector<std::atomic<unsigned>> counters;
    static void run(Fn *fn);
    template <typename... Vs> static void await(Vs &...vs);

  protected:
    static std::monostate main(std::atomic<Fn *> *ref);
    static Fn *get_work();
    template <typename... Vs> static bool all_done(Vs &&...vs) {
        return (... && vs->done());
    }
};

struct stack_inversion : public std::exception {};
