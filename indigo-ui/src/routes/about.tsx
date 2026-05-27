import { A } from "@solidjs/router";

export default function About() {
  return (
    <main class="max-w-2xl mx-auto p-8 text-center">
      <h1 class="text-3xl font-bold text-white font-data tracking-wider mb-4">About Indigo</h1>
      <p class="text-ng-secondary mb-6">
        Decentralized AI inference platform. Built with SolidJS.
      </p>
      <p class="text-ng-muted text-sm font-data">
        <A href="/" class="text-cyan-ng hover:text-magenta-ng transition-colors">
          Home
        </A>
      </p>
    </main>
  );
}
