#pragma once

#include "data_structures/lazy.hpp"
#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "system/work_manager_pre.hpp"
#include "time/sleep.hpp"

#include <atomic>
#include <chrono>
#include <cstdint>
#include <deque>
#include <tuple>
#include <type_traits>
#include <vector>

using namespace std::literals::chrono_literals;

class Fn {
    friend struct WorkManager;

  protected:
    virtual void run() = 0;
    virtual bool done() const = 0;

  public:
    virtual ~Fn() = default;
    virtual void call() {
        WorkManager::queue.acquire();
        WorkManager::queue->push_back(this);
        WorkManager::queue.release();
    }
    template <typename... Ts> auto static reference_all(Ts... args) {
        return std::make_tuple(new LazyConstant<std::decay_t<Ts>>(args)...);
    }
    template <typename T, typename... Ts>
    static void initialize(T *&fn, Ts &&...args) {
        if (fn == nullptr) {
            fn = new T(std::forward<Ts>(args)...);
        }
    }
};

template <typename Ret, typename... Args>
struct ParametricFn : public Fn, Lazy<Ret> {
    using ArgsT = std::tuple<std::add_pointer_t<Lazy<std::decay_t<Args>>>...>;
    using R = std::decay_t<Ret>;
    std::atomic<bool> done_flag{false};
    Locked<std::vector<Continuation>> continuations;
    ArgsT args;
    R ret;
    ParametricFn() = default;

    explicit ParametricFn(
        std::add_pointer_t<
            Lazy<std::decay_t<Args>>>... args) requires(sizeof...(Args) > 0)
        : args(args...) {}

    explicit ParametricFn(std::add_const_t<std::add_lvalue_reference_t<
                              Args>>... args) requires(sizeof...(Args) > 0)
        : args(reference_all(args...)) {}
    virtual ParametricFn<Ret, Args...> *clone() const = 0;
    virtual Lazy<R> *body(std::add_lvalue_reference_t<
                          std::add_pointer_t<Lazy<std::decay_t<Args>>>>...) = 0;
    void run() override {
        Lazy<R> *return_ =
            std::apply([this](auto &&...t) { return body(t...); }, args);
        WorkManager::await(return_);
        ret = return_->value();
        continuations.acquire();
        for (const Continuation &c : *continuations) {
            Lazy<Ret>::update_continuation(c);
        }
        continuations->clear();
        done_flag.store(true, std::memory_order_relaxed);
        continuations.release();
    }
    bool done() const override {
        return done_flag.load(std::memory_order_relaxed);
    }
    R value() override { return ret; }
    void add_continuation(Continuation c) override {
        continuations.acquire();
        if (done()) {
            continuations.release();
            Lazy<Ret>::update_continuation(c);
        } else {
            continuations->push_back(c);
            continuations.release();
        }
    }
};

template <typename F, typename R, typename... A>
struct EasyCloneFn : ParametricFn<R, A...> {
    using ParametricFn<R, A...>::ParametricFn;
    ParametricFn<R, A...> *clone() const override { return new F{}; }
};

class FinishWork : public Fn {
    void run() override{};
    bool done() const override { return true; };
};

template <typename T> struct BlockFn : public ParametricFn<T> {
    std::function<Lazy<T> *()> body_fn;
    Lazy<T> *body() override { return body_fn(); }
    explicit BlockFn(std::function<Lazy<T> *()> &&f) : body_fn(std::move(f)){};
    explicit BlockFn(const std::function<Lazy<T> *()> &f) : body_fn(f){};
    ParametricFn<T> *clone() const override { return new BlockFn<T>{body_fn}; }
};

#include "system/work_manager.hpp"
