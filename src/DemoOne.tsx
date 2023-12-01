import { ResponsiveLineCanvas } from "@nivo/line";
import { fs } from "@tauri-apps/api";
import { BaseDirectory } from "@tauri-apps/api/fs";
import { useState, useCallback, useEffect } from "react";

type EcgReading = {
    timestamp: Date,
    value: number;
};

const parseEcgData = (raw: string): EcgReading[] => {
    const lines = raw.split("\n");

    const splitLines = lines.map(line => line.split(","));

    // const sanitizeTimeStampForm = (raw: string) => raw.slice(2, raw.length - 2)
    // use with samples 3, 4
    const sanitizeTimeStampForm = (raw: string) => `00:0${raw.slice(1, raw.length - 1)} 01/01/2011`;

    // console.log(lines);
    const returnData = splitLines.slice(2).map(splitLine => ({
        timestamp: new Date(sanitizeTimeStampForm(splitLine[0])),
        value: parseFloat(splitLine[2])
    })
    );

    return returnData;
};

export const DemoOne = () => {
    const [ecgData, setEcgData] = useState<EcgReading[]>([]);
    const [traditionalData, setTraditionalData] = useState<{ sweepLength: number, data: EcgReading[]; }>({ sweepLength: 0, data: [] });
    const [timeDelta, setTimeDelta] = useState(0);
    const [chartBounds, setChartBounds] = useState<{ min: number, max: number; }>({ min: Infinity, max: -Infinity });

    const INTERVAL_SECONDS = 10;
    const tickValues = `every ${INTERVAL_SECONDS} seconds`;

    const adjustChartBounds = useCallback((value: number) => {
        setChartBounds(old => {
            if (value > old.max) {
                return ({ ...old, max: value });
            }

            if (value < old.min) {
                return ({ ...old, min: value });
            }

            return old;
        });
    }, [chartBounds]);

    useEffect(() => {
        (async () => {
            const then = Date.now();

            const fileData = await fs.readTextFile("samples (4).csv", { dir: BaseDirectory.Download });

            const ecgData = parseEcgData(fileData);
            ecgData.forEach(record => adjustChartBounds(record.value));
            setEcgData(ecgData);

            console.log("Time elapsed to process all data:", Date.now() - then);
        })();

        let then = Date.now();

        const handle = setInterval(() => {
            const now = Date.now();
            const dif = now - then;
            setTimeDelta(old => (old + dif));
            then = now;
        }, 1);

        return () => clearInterval(handle);
    }, []);

    useEffect(() => {
        setTraditionalData(old => {
            const lower = Math.floor(timeDelta / (INTERVAL_SECONDS * 1000)) * INTERVAL_SECONDS * 1000;
            const upper = timeDelta;

            const left = ecgData.filter(reading => {
                const readingMillis = reading.timestamp.getTime() - ecgData[0].timestamp.getTime();

                return readingMillis >= lower && readingMillis < upper;
            }).map(reading => ({ ...reading, timestamp: new Date(reading.timestamp.getTime() - lower) }));

            return { sweepLength: left.length, data: [...left, ...old.data.slice(left.length)] };
        });
    }, [timeDelta]);

    const constrainToTimeRange = (arr: EcgReading[]) => {
        const RANGE_MILLIS = INTERVAL_SECONDS * 1_000;

        return arr.filter(reading => reading.timestamp.getTime() > arr[arr.length - 1].timestamp.getTime() - RANGE_MILLIS);
    };

    const selected = ecgData.filter(reading => reading.timestamp.getTime() < ecgData[0].timestamp.getTime() + timeDelta);
    const data = constrainToTimeRange(selected).map(reading => ({ x: reading.timestamp, y: reading.value }));

    const commonProperties = {
        margin: { top: 20, right: 20, bottom: 60, left: 80 },
        data,
        animate: false,
        enableSlices: 'x',
    };

    const traditional = traditionalData.data.map((reading, i) => {
        const yValue = (i >= traditionalData.sweepLength && i <= traditionalData.sweepLength + 20) ? null : reading.value;
        return ({ x: reading.timestamp, y: yValue });
    });

    return (
        <div className="flex flex-col w-full h-full">
            <div className="w-full h-44">
                <ResponsiveLineCanvas
                    {...commonProperties}
                    margin={{ top: 30, right: 50, bottom: 60, left: 50 }}
                    data={[
                        { id: 'A', data: data },
                    ]}
                    xScale={{ type: 'time', format: 'native' }}
                    yScale={{ type: 'linear', max: chartBounds.max, min: chartBounds.min }}
                    axisTop={{
                        tickValues,
                        format: "%H:%M:%S",
                    }}
                    axisBottom={{
                        tickValues,
                        format: "%H:%M:%S",
                    }}
                    axisRight={{}}
                    colors={"#36ea5b"}
                    enablePoints={false}
                    enableGridX={true}
                    curve="monotoneX"
                    isInteractive={false}
                    enableSlices={false}
                    theme={{
                        axis: { ticks: { text: { fontSize: 14 } } },
                        grid: { line: { stroke: '#ddd', strokeDasharray: '1 2' } },
                    }}
                />
            </div>
            <div className="w-full h-44">
                <ResponsiveLineCanvas
                    {...commonProperties}
                    margin={{ top: 30, right: 50, bottom: 60, left: 50 }}
                    data={[
                        {
                            id: 'A', data: traditional
                        },
                    ]}
                    xScale={{ type: 'time', max: ecgData[0] ? new Date(ecgData[0].timestamp.getTime() + INTERVAL_SECONDS * 1000) : new Date() }}
                    yScale={{ type: 'linear', max: chartBounds.max, min: chartBounds.min }}
                    axisTop={{
                        tickValues,
                        format: "%H:%M:%S",
                    }}
                    axisBottom={{
                        tickValues,
                        format: "%H:%M:%S",
                    }}
                    axisRight={{}}
                    colors={"#63d8fa"}
                    enablePoints={false}
                    enableGridX={false}
                    curve="linear"
                    isInteractive={false}
                    enableSlices={false}
                    theme={{
                        axis: { ticks: { text: { fontSize: 14, color: 'white' } } },
                        grid: { line: { stroke: '#ddd', strokeDasharray: '1 2' } },
                    }}
                />
            </div>
        </div>
    );
};