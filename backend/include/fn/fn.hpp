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

template <typename Ret, typename... Args> struct TypedWeakFn;
template <typename Ret, typename... Args> struct TypedFn : public Fn {
    friend class TypedWeakFn<Ret, Args...>;
    using T = Ret (*)(Args..., const std::shared_ptr<void>);
    TypedFn(T fn, const std::shared_ptr<void> env);
    explicit TypedFn(T fn);
    TypedFn();
    T fn() const;
    Ret call(Args... args) const;
};

template <typename E, typename Ret, typename... Args>
struct TypedClosure : public TypedFn<Ret, Args...> {
    using T = Ret (*)(Args..., std::shared_ptr<E>);
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

template <typename Ret, typename... Args> struct TypedWeakFn : public WeakFn {
    explicit TypedWeakFn(TypedFn<Ret, Args...> f);
    TypedWeakFn();
    TypedFn<Ret, Args...> lock() const;
};
