#pragma once

#include <memory>
#include <type_traits>

struct WeakFn;
class Fn {
    friend class WeakFn;

  protected:
    void *_fn = nullptr;
    std::shared_ptr<void> _env;
    Fn(void *fn, std::shared_ptr<void> env);
    explicit Fn(void *fn);

  public:
    Fn();
    virtual ~Fn();
};

template <typename R, typename... Args> struct TypedWeakFn;
template <typename R, typename... Args> struct TypedFn : public Fn {
    friend class TypedWeakFn<R, Args...>;
    using T = R (*)(Args..., const std::shared_ptr<void>);
    TypedFn(T fn, const std::shared_ptr<void> env);
    explicit TypedFn(T fn);
    TypedFn();
    T fn() const;
    R call(Args... args) const;
};

template <typename E, typename R, typename... Args>
struct TypedClosure : public TypedFn<R, Args...> {
    using T = R (*)(Args..., std::shared_ptr<E>);
    TypedClosure(T fn, E env);
    explicit TypedClosure(T fn);
    E &env();
};

class WeakFn {
  protected:
    void *_fn = nullptr;
    std::weak_ptr<void> _env;

  public:
    explicit WeakFn(Fn f);
    WeakFn();
    Fn lock() const;
};

template <typename R, typename... Args> struct TypedWeakFn : public WeakFn {
    explicit TypedWeakFn(TypedFn<R, Args...> f);
    TypedWeakFn();
    TypedFn<R, Args...> lock() const;
};
