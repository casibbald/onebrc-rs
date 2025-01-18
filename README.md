### Results using test data

```bash
❯ cat data/weather_stations.csv| wc -l
44693
```

```bash
❯ ./target/release/onebrc-rs data/weather_stations.csv # 
Total execution time: 67.096917ms
Total execution time: 74.611375ms
Total execution time: 78.083208ms

```

### Results using real data

The following results are from reading the entire file into memory and then processing.

```bash
❯ ./target/release/onebrc-rs --file=data/measurements.txt
Total execution time: 401.212742334s
Total execution time: 390.693394583s
Total execution time: 386.078361042s

```



