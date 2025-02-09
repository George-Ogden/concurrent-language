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
    static inline Locked<std::deque<std::weak_ptr<Fn>>> queue;
    static std::shared_ptr<Fn> finish_work;
    static inline std::vector<std::atomic<unsigned>> counters;
    static void call(std::shared_ptr<Fn> fn);
    static void run(std::shared_ptr<Fn> fn);
    template <typename... Vs> static void await(Vs &...vs);
    template <typename... Vs> static void await_all(Vs &...vs);

  protected:
    template <typename... Vs> static void await_restricted(Vs &...vs);
    static std::monostate main(std::atomic<std::shared_ptr<Fn>> *ref);
    static std::shared_ptr<Fn> get_work();
    template <typename... Vs> static bool all_done(Vs &&...vs) {
        return (... && vs->done());
    }
};

struct stack_inversion : public std::exception {
    stack_inversion() = default;
    const char *what() const noexcept override { return "Stack inversion"; }
};
struct finished : public std::exception {};
