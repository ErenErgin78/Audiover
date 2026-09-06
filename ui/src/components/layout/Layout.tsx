import Header from "./Header";
import Sidebar from "./Sidebar";
import type { ReactNode } from "react";

interface LayoutProps {
  children: ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  return (
    <div className="flex flex-col" style={{ height: "100vh" }}>
      <Header />
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main
          className="flex-1 overflow-y-auto overflow-x-hidden"
          style={{ background: "var(--bg-base)" }}
        >
          {children}
        </main>
      </div>
    </div>
  );
}
