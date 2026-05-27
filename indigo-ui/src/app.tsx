import { Router } from "@solidjs/router";
import { FileRoutes } from "@solidjs/start/router";
import { Suspense, ErrorBoundary } from "solid-js";
import Nav from "~/components/Nav";
import NeuralBg from "~/components/NeuralBg";
import "./app.css";

export default function App() {
  return (
    <Router
      root={props => (
        <>
          <NeuralBg />
          <Nav />
          <ErrorBoundary fallback={(err) => <div class="p-8 text-center text-red-400">Something went wrong: {err.toString()}</div>}>
            <Suspense fallback={
              <div class="flex h-[calc(100vh-64px)] items-center justify-center">
                <div class="flex flex-col items-center gap-4">
                  <div class="w-10 h-10 border-2 border-cyan-ng/20 border-t-cyan-ng animate-spin" style="clip-path: var(--ng-clip-card-sm)"></div>
                  <span class="font-data text-xs uppercase tracking-widest text-ng-muted">Initializing...</span>
                </div>
              </div>
            }>
              {props.children}
            </Suspense>
          </ErrorBoundary>
        </>
      )}
    >
      <FileRoutes />
    </Router>
  );
}
