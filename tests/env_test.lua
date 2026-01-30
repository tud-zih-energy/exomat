freqs = from_list("FREQ", {1000, 2000, 3000})
kernels = from_output("KERNELS", "add\nmul\ndiv")
cpus = from_list("CPUS", {"0,1", "0,1,2,3"})

result0 = freqs + freqs

result1 = cross({freqs, cpus, kernels})

result2 = cross({freqs, cpus, kernels, from_list("TURBO", {"OFF"})}) + cross({from_list("FREQ", {3000}), cpus, kernels, from_list("TURBO", {"ON"})})


