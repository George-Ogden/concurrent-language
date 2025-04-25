for O_N in "O1 2" "Ofast 1" "O3 1"; do
for NUM_CPUS in 4 2 1; do
    for OPT in Ofast O3 O2 O1; do
        sed -Ei "s/^(OPTIMIZATION :=).*/\1 -$OPT/" backend/Makefile ;
        make benchmark
        FOLDER=$(ls -lt logs | head -2 | tail -1 | awk '{ print $NF }')
        echo $OPT $NUM_CPUS cpus > "logs/$FOLDER/title.txt"
    done
done
