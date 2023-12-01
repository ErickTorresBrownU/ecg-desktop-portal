import "./App.css";
import { DemoOne } from "./DemoOne";
import { DemoTwo } from "./DemoTwo";

const App = () => {
    return (
        <div className="w-full h-screen overflow-hidden p-5 bg-black">
            <div className="flex flex-col w-full h-full">
                {/* <DemoOne /> */}
                <DemoTwo />
            </div>
        </div>
    );
};

export default App;