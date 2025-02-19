#pragma once

#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "types/utils.hpp"

#include <utility>

struct Work;
template <typename T> class Lazy {
  public:
    virtual bool done() const = 0;
    virtual T value() const = 0;
    virtual T &lvalue() = 0;
    virtual void add_continuation(Continuation c) = 0;
    virtual ~Lazy();
};

template <typename T> class LazyConstant : public Lazy<T> {
    T _value;

  public:
    template <typename... Args> LazyConstant(Args &&...);
    bool done() const override;
    T value() const override;
    T &lvalue() override;
    void add_continuation(Continuation c) override;
};

template <typename T> class LazyWork : public Lazy<T> {
    friend class Work;
    T _value;
    std::shared_ptr<Work> work;

  public:
    LazyWork();
    bool done() const override;
    T value() const override;
    T &lvalue() override;
    void add_continuation(Continuation c) override;
};

template <typename T> class LazyPlaceholder : public Lazy<T> {
    std::shared_ptr<Lazy<T>> reference = nullptr;
    std::shared_ptr<Work> work;
    Locked<std::vector<Continuation>> continuations;

  public:
    explicit LazyPlaceholder(std::shared_ptr<Work> work);
    void add_continuation(Continuation c) override;
    void assign(std::shared_ptr<Lazy<T>> value);
    bool done() const override;
    T value() const override;
    T &lvalue() override;
};

template <typename T, typename... Args>
std::shared_ptr<Lazy<T>> make_lazy(Args &&...args) {
    return std::make_shared<LazyConstant<T>>(std::forward<Args>(args)...);
}
