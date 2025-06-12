"use client";
import { Geist, Geist_Mono } from "next/font/google";
import "@/styles/globals.css";
import Link from 'next/link'; // Added import for Link

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased font-sans bg-gray-50 text-gray-800`}
      >
        <nav className="bg-gray-800 text-white shadow-md">
          <div className="container mx-auto px-4">
            <div className="flex items-center justify-between h-16">
              <div className="flex items-center">
                <Link href="/" className="font-bold text-xl hover:text-gray-300 transition-colors">
                  Rust LLM UI
                </Link>
              </div>
              <div className="flex space-x-4">
                <Link href="/" className="px-3 py-2 rounded-md text-sm font-medium hover:bg-gray-700 hover:text-gray-200 transition-colors">
                  Home
                </Link>
                <Link href="/projects" className="px-3 py-2 rounded-md text-sm font-medium hover:bg-gray-700 hover:text-gray-200 transition-colors">
                  Projects
                </Link>
                <Link href="/query" className="px-3 py-2 rounded-md text-sm font-medium hover:bg-gray-700 hover:text-gray-200 transition-colors">
                  Query Docs
                </Link>
              </div>
            </div>
          </div>
        </nav>
        <main className="mt-4"> {/* Added main tag and margin-top for content separation */}
          {children}
        </main>
      </body>
    </html>
  );
}
