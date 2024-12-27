"use client"
/* eslint-disable @typescript-eslint/no-explicit-any */
import React from 'react';
import { Layout, SessionProvider, SnackbarProvider } from 'comps';
import { nhost } from 'nhost';
import { NhostProvider } from '@nhost/nextjs';
import { LocalizationProvider } from '@mui/x-date-pickers';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import M3ThemeProvider from 'core/theme/M3ThemeProvider';
import ThemeModeProvider from 'core/theme/ThemeModeContext';
import ThemeSchemeProvider from 'core/theme/ThemeSchemeContext';
import { Analytics } from '@vercel/analytics/react';
import { ErrorBoundary } from 'react-error-boundary';
import { AppRouterCacheProvider } from '@mui/material-nextjs/v13-appRouter';

const fallbackRender = ({ error }: { error: { stack: string } }) => {
  return (
    <div role="alert">
      <h3>Noget gik galt! 😔</h3>
      <p>
        Send venligst følgende besked til{' '}
        <a href="mailto:niclas@overby.me">niclas@overby.me</a>:
      </p>
      <pre style={{ background: '#EDEDED', padding: '20px' }}>
        {error.stack}
      </pre>
    </div>
  );
};

const RootLayout = ({
  children
}: {
  children: JSX.Element
}) => (
  <html>
    <body>
      <ErrorBoundary fallbackRender={fallbackRender}>
        <AppRouterCacheProvider>
          <NhostProvider nhost={nhost}>
            <LocalizationProvider dateAdapter={AdapterDateFns}>
              <SessionProvider>
                <ThemeModeProvider>
                  <ThemeSchemeProvider>
                    <M3ThemeProvider>
                      <SnackbarProvider>
                        <Layout>
                          {children}
                        </Layout>
                      </SnackbarProvider>
                    </M3ThemeProvider>
                  </ThemeSchemeProvider>
                </ThemeModeProvider>
              </SessionProvider>
            </LocalizationProvider>
          </NhostProvider>
          <Analytics />
        </AppRouterCacheProvider>
      </ErrorBoundary>
    </body>
  </html>
);

export default RootLayout;
