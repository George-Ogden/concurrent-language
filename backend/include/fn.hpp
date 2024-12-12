#pragma once

#include <tuple>
#include <type_traits>

class Fn {
  protected:
    virtual void body() = 0;

  public:
    void run() { body(); }
};

template <typename Ret, typename... Args> struct ParametricFn : public Fn {
    using ArgsT = std::tuple<std::add_pointer_t<Args>...>;
    ArgsT args;
    Ret *ret;
};
