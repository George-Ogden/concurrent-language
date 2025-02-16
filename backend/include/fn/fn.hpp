#pragma once

#include <memory>
#include <type_traits>

class Fn {
  protected:
    const void *_fn = nullptr;
    const std::shared_ptr<void> _env;
    Fn(void *fn, std::shared_ptr<void> env);

  public:
    Fn();
    virtual ~Fn();
};

template <typename R, typename... Args> struct TypedFn : public Fn {
    using T = R (*)(Args..., const std::shared_ptr<void>);
    TypedFn(T fn, const std::shared_ptr<void> env);
    T fn() const;
    R call(Args... args) const;
};

template <typename E, typename R, typename... Args>
struct TypedClosure : public TypedFn<R, Args...> {
    using T = R (*)(Args..., std::shared_ptr<E>);
    TypedClosure(T fn, std::shared_ptr<E> env);
    const std::shared_ptr<E> env() const;
};
