#include "main/main.hpp"
#include "system/work_manager.hpp"

#include <chrono>
#include <iostream>

int main(int argc, char *argv[]) {
    auto start = std::chrono::high_resolution_clock::now();
    using ArgsT = remove_lazy_t<Main::ArgsT>;
    argc--;
    argv++;
    constexpr auto N = std::tuple_size_v<ArgsT>;
    if (N != argc) {
        std::cerr << "Invalid number of arguments expected " << N << " got "
                  << argc << "." << std::endl;
        exit(1);
    }

    ArgsT args = [&argv]<std::size_t... Is>(std::index_sequence<Is...>) {
        return std::make_tuple(
            std::make_shared<
                LazyConstant<remove_lazy_t<std::tuple_element_t<Is, ArgsT>>>>(
                convert_arg<remove_lazy_t<std::tuple_element_t<Is, ArgsT>>>(
                    argv[Is]))...);
    }
    (std::make_index_sequence<N>{});

    std::shared_ptr<Main> main = std::make_shared<Main>();
    main->args = args;
    WorkManager::run(main);

    auto end = std::chrono::high_resolution_clock::now();

    std::cout << main->value() << std::endl;

    auto duration =
        std::chrono::duration_cast<std::chrono::nanoseconds>(end - start);
    std::cerr << "Execution time: " << duration.count() << " ns" << std::endl;

    return 0;
}
