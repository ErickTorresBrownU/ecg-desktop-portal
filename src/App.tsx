import "./App.css";
import { Demo } from "./Demo";

const App = () => {
    return (
        <div className="w-full h-screen overflow-hidden p-5 bg-black">
            <div className="flex flex-col w-full h-full">
                <Demo />
            </div>
        </div>
    );
};

export default App;