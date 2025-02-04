#pragma once

#include "data_structures/lazy.hpp"
#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "system/work_manager_pre.hpp"
#include "time/sleep.hpp"
#include "types/builtin.hpp"
#include "types/utils.hpp"

#include <atomic>
#include <chrono>
#include <cstdint>
#include <deque>
#include <memory>
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
    template <typename... Ts> auto static reference_all(Ts... args) {
        return std::make_tuple(
            std::make_shared<LazyConstant<std::decay_t<Ts>>>(args)...);
    }
};

template <typename Ret, typename... Args>
struct ParametricFn : public Fn, Lazy<Ret> {
    using ArgsT = LazyT<std::tuple<std::decay_t<Args>...>>;
    using R = std::decay_t<Ret>;
    std::atomic<bool> done_flag{false};
    Locked<std::vector<Continuation>> continuations;
    ArgsT args;
    LazyT<R> ret;
    ParametricFn() = default;

    explicit ParametricFn(LazyT<std::decay_t<Args>>... args) requires(
        sizeof...(Args) > 0)
        : args(args...) {}

    explicit ParametricFn(std::add_const_t<std::add_lvalue_reference_t<
                              Args>>... args) requires(sizeof...(Args) > 0)
        : args(reference_all(args...)) {}
    virtual ~ParametricFn() { cleanup_args(); }
    virtual std::shared_ptr<ParametricFn<Ret, Args...>> clone() const = 0;
    virtual std::tuple<std::shared_ptr<ParametricFn<Ret, Args...>>, LazyT<Ret>>
    clone_with_args(LazyT<std::decay_t<Args>>... args) const {
        std::shared_ptr<ParametricFn<Ret, Args...>> call = this->clone();
        call->args = std::make_tuple(args...);
        call->ret = Lazy<LazyT<R>>::make_placeholders();
        return std::make_tuple(call, call->ret);
    };
    virtual LazyT<R>
    body(std::add_lvalue_reference_t<LazyT<std::decay_t<Args>>>...) = 0;
    void run() override {
        auto arguments = this->args;
        if (!done_flag.load(std::memory_order_acquire)) {
            LazyT<R> return_ = std::apply(
                [this](auto &&...t) { return body(t...); }, arguments);
            WorkManager::await(return_);
            assign(ret, return_);
        }
        continuations.acquire();
        for (const Continuation &c : *continuations) {
            Lazy<R>::update_continuation(c);
        }
        continuations->clear();
        done_flag.store(true, std::memory_order_release);
        cleanup();
        continuations.release();
    }
    template <typename T> static void assign(T &ret, T &return_) {
        if constexpr (is_lazy_v<T>) {
            auto ptr =
                std::dynamic_pointer_cast<LazyPlaceholder<remove_lazy_t<T>>>(
                    ret);
            if (ptr == nullptr) {
                ret = return_;
            } else {
                ptr->assign(return_);
            }
        } else if constexpr (is_tuple_v<T>) {
            constexpr auto size = std::tuple_size_v<T>;
            [&]<std::size_t... Is>(std::index_sequence<Is...>) {
                ((assign(std::get<Is>(ret), std::get<Is>(return_))), ...);
            }
            (std::make_index_sequence<size>{});
        }
    }
    virtual void cleanup() { cleanup_args(); }
    void cleanup_args() { this->args = ArgsT{}; }
    bool done() const override {
        return done_flag.load(std::memory_order_relaxed);
    }
    R value() const override { return Lazy<R>::extract_value(ret); }
    void add_continuation(Continuation c) override {
        continuations.acquire();
        if (done()) {
            continuations.release();
            Lazy<R>::update_continuation(c);
        } else {
            continuations->push_back(c);
            continuations.release();
        }
    }
};

template <typename F, typename R, typename... A>
struct EasyCloneFn : ParametricFn<R, A...> {
    using ParametricFn<R, A...>::ParametricFn;
    std::shared_ptr<ParametricFn<R, A...>> clone() const override {
        return std::make_shared<F>();
    }
};

class FinishWork : public Fn {
    void run() override{};
    bool done() const override { return true; };
};

template <typename T> struct BlockFn : public ParametricFn<T> {
    std::function<LazyT<T>()> body_fn;
    LazyT<T> body() override { return body_fn(); }
    explicit BlockFn(std::function<LazyT<T>()> &&f) : body_fn(std::move(f)){};
    explicit BlockFn(const std::function<LazyT<T>()> &f) : body_fn(f){};
    std::shared_ptr<ParametricFn<T>> clone() const override {
        return std::make_shared<BlockFn<T>>(body_fn);
    }
};

template <typename E> struct ClosureRoot {
    LazyT<E> env;
    explicit ClosureRoot(const LazyT<E> &e) : env(e) {}
    explicit ClosureRoot() = default;
    virtual ~ClosureRoot() = default;
};

template <typename T, typename E, typename R, typename... A>
struct Closure : ClosureRoot<E>, ParametricFn<R, A...> {
    using ClosureRoot<E>::ClosureRoot;
    std::shared_ptr<ParametricFn<R, A...>> clone() const override {
        return std::make_shared<T>(this->env);
    }
    virtual ~Closure() {}
};

template <> struct ClosureRoot<Empty> {};

template <typename T, typename R, typename... A>
struct Closure<T, Empty, R, A...> : ClosureRoot<Empty>, ParametricFn<R, A...> {
    explicit Closure() {}
    explicit Closure(const Empty &e) {}
    std::shared_ptr<ParametricFn<R, A...>> clone() const override {
        return std::make_shared<T>();
    }
};

#include "system/work_manager.hpp"
