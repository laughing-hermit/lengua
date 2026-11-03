import type { Metadata } from "next";
import type { ReactNode } from "react";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: "lengua specs",
  description: "Share what a program does, never its code: every computer compiles and checks its own.",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>
        <header className="top">
          <Link href="/" className="brand">
            lengua
          </Link>
          <nav>
            <Link href="/specs">Specs</Link>
            <Link href="/share">Share a spec</Link>
          </nav>
        </header>
        <main>{children}</main>
        <footer>Specs only, never code. Each computer compiles and checks its own.</footer>
      </body>
    </html>
  );
}
