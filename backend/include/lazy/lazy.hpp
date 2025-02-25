#pragma once

#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "types/utils.hpp"

#include <atomic>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

struct Work;
struct LazyValue {
    virtual void require() = 0;
    virtual ~LazyValue();
};

template <typename T> struct Lazy : LazyValue {
    virtual bool done() = 0;
    virtual T value() = 0;
    virtual T &lvalue() = 0;
    virtual void add_continuation(Continuation c) = 0;
    virtual void require() override;
    virtual std::shared_ptr<Lazy<T>> as_ref();
};

template <typename T> class LazyConstant : public Lazy<T> {
    T _value;

  public:
    template <typename... Args> LazyConstant(Args &&...);
    bool done() override;
    T value() override;
    T &lvalue() override;
    void add_continuation(Continuation c) override;
};

template <typename T> class LazyPlaceholder : public Lazy<T> {
    std::atomic<std::shared_ptr<Lazy<T>>> reference = nullptr;
    std::atomic<std::shared_ptr<Work>> work;
    bool required = false;
    Locked<std::vector<Continuation>> continuations;

  public:
    explicit LazyPlaceholder(std::shared_ptr<Work> work);
    void add_continuation(Continuation c) override;
    void assign(std::shared_ptr<Lazy<T>> value);
    bool done() override;
    T value() override;
    T &lvalue() override;
    void require() override;
    std::shared_ptr<Lazy<T>> as_ref() override;
};

template <typename T, typename... Args>
std::shared_ptr<Lazy<T>> make_lazy(Args &&...args) {
    return std::make_shared<LazyConstant<T>>(std::forward<Args>(args)...);
}
