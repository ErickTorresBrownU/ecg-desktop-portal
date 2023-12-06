import { ResponsiveLineCanvas } from "@nivo/line";
import { listen } from "@tauri-apps/api/event";
import { useState, useCallback, useEffect } from "react";
import { ZScore } from "./ZScore";

type EcgReading = {
    milliseconds: number,
    value: number;
};

export const Demo = () => {
    const [ecgData, setEcgData] = useState<EcgReading[]>([]);
    const [traditionalData, setTraditionalData] = useState<{ sweepLength: number, data: EcgReading[]; }>({ sweepLength: 0, data: [] });
    const [timeElapsed, setTimeElapsed] = useState(0);
    const [chartBounds, setChartBounds] = useState<{ min: number, max: number; }>({ min: Infinity, max: -Infinity });

    const INTERVAL_SECONDS = 15;
    const INTERVAL_MILLIS = INTERVAL_SECONDS * 1_000;
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

    const cullEcgData = useCallback(() => {
        setEcgData(old => {
            if (old.length == 0) {
                return old;
            }

            let idx = old.findIndex((value) => value.milliseconds > old[old.length - 1].milliseconds - INTERVAL_MILLIS);

            return old.slice(idx);
        });
    }, [ecgData]);

    useEffect(() => {
        let then = Date.now();

        const handle = setInterval(() => {
            const now = Date.now();
            const dif = now - then;
            setTimeElapsed(old => (old + dif));
            then = now;
        }, 1);

        const newReadingListenerHandle = listen("new-reading", (event) => {
            let payload = event.payload as any as EcgReading;

            adjustChartBounds(payload.value);
            setEcgData(old => [...old, { milliseconds: payload.milliseconds, value: payload.value }]);
            // cullEcgData();
        });

        const monitorResetListenerHandle = listen("reset-monitor", () => {
            setEcgData([]);
            setTraditionalData({ sweepLength: 0, data: [] });
            setTimeElapsed(0);
        });

        return () => {
            newReadingListenerHandle.then(unlisten => unlisten());
            monitorResetListenerHandle.then(unlisten => unlisten());
            clearInterval(handle);
        };
    }, []);

    useEffect(() => {
        setTraditionalData(old => {
            if (ecgData.length == 0) {
                return old;
            }

            const lowerBound = Math.floor(timeElapsed / INTERVAL_MILLIS) * INTERVAL_MILLIS + ecgData[0].milliseconds;
            const upperBound = timeElapsed + ecgData[0].milliseconds;

            let latest = 0;

            const left: EcgReading[] = ecgData.filter(({ milliseconds }) => {
                return milliseconds >= lowerBound && milliseconds <= upperBound;
            }).map(reading => {
                const newMillis = reading.milliseconds - lowerBound;
                latest = newMillis;
                return ({
                    ...reading,
                    milliseconds: newMillis
                });
            });

            return {
                sweepLength: left.length,
                data: [...left, ...old.data.filter(reading => reading.milliseconds > latest)]
            };
        });
    }, [timeElapsed]);

    const constrainToTimeRange = (arr: EcgReading[]) => {
        const constrained = arr.filter(reading => reading.milliseconds < arr[0].milliseconds + timeElapsed && reading.milliseconds > arr[arr.length - 1].milliseconds - INTERVAL_MILLIS);

        return constrained;
    };

    const data = constrainToTimeRange(ecgData).map(reading => ({ x: new Date(reading.milliseconds), y: reading.value }));
    const signals = (() => {
        let lag: number = 30;
        let threshold: number = 10;
        let influence: number = 0.25;

        return ZScore.calc(data.map(reading => reading.y), lag, threshold, influence).signals;
    })();

    const heartRate = (() => {
        type Signal = number;
        type Index = number;
        type SignalIndexPair = [Signal, Index];

        const peaks: SignalIndexPair[] = signals.map((signal, i) => [signal, i] as SignalIndexPair).filter(signalIndexPair => signalIndexPair[0] === 1);

        if (peaks.length < 2) {
            return undefined;
        }

        const deltaSeconds = (data[peaks[peaks.length - 1][1]].x.getTime() - data[peaks[0][1]].x.getTime()) / 1000;

        const bpm = Math.round(peaks.length * (60 / deltaSeconds));

        return bpm >= 300 ? undefined : bpm;
    })();

    const commonProperties = {
        margin: { top: 20, right: 20, bottom: 60, left: 80 },
        data,
        animate: false,
    };

    const traditional = traditionalData.data.map((reading) => {
        const last = traditionalData.sweepLength - 1;
        const endTime = traditionalData.data[last < 0 ? 0 : last].milliseconds;

        const yValue = (reading.milliseconds > endTime && reading.milliseconds <= endTime + (0.025 * INTERVAL_MILLIS)) ? null : reading.value;
        const DEBUG = false;

        return ({ x: new Date(reading.milliseconds), y: DEBUG ? reading.value : yValue });
    });

    return (
        <div className="flex flex-row w-full h-52">
            <div className="flex flex-col w-full h-full">
                <div className="flex-grow h-full">
                    <ResponsiveLineCanvas
                        {...commonProperties}
                        margin={{ top: 30, right: 50, bottom: 60, left: 50 }}
                        data={[
                            {
                                id: 'A', data: traditional, color: "#0000ff"
                            }
                        ]}
                        xScale={{ type: 'time', min: new Date(0), max: new Date(INTERVAL_MILLIS) }}
                        yScale={{ type: 'linear', max: chartBounds.max, min: chartBounds.min }}
                        axisTop={{
                            tickValues,
                            format: "%M:%S",
                        }}
                        axisBottom={{
                            tickValues,
                            format: "%M:%S",
                        }}
                        axisLeft={null}
                        axisRight={null}
                        colors={"#63d8fa"}
                        enablePoints={false}
                        enableGridX={false}
                        enableGridY={false}
                        curve="linear"
                        isInteractive={false}
                        enableSlices={false}
                    />
                </div>
            </div>
            <div className="flex flex-row w-80 h-full border-0 border-gray-300 rounded-sm space-x-4">
                <p className="text-[#36ea5b] text-xl">HR</p>
                <h1 className="text-[#36ea5b] text-9xl font-mono font-semibold whitespace-nowrap">{heartRate ?? "---"}</h1>
            </div>
        </div>
    );
};