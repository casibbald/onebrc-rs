## Results using test data

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

## Results using real data

### Generating real data

```bash
pip install -r hack/requirements.txt
cd data/
python hack/createMeasurements.py
```

### Results using real data with simple readlines implementation
```bash
The following results are from reading the entire 1br file into memory and then processing.

```bash
❯ ./target/release/onebrc-rs --file=data/measurements.txt
Total execution time: 401.212742334s
Total execution time: 390.693394583s
Total execution time: 386.078361042s
```


### Results after adding coroutines and memory map

```bash
Total execution time: 265.518331958s
```

Results currently constrained on my macbook due to insufficient memory and having to swap very heavily.
Next optimization will need to be a better implementation of pipelining using coroutine and memory maps.

A threading + coroutine approach was attempted but the overhead of the threading was too high and the performance was worse than the simple readlines implementation.

