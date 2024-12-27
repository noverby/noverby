import { createTheme, ThemeOptions, useMediaQuery } from '@mui/material';
import { red } from '@mui/material/colors';
import { useSession } from 'hooks';

export const theme: ThemeOptions = {
  typography: {
    fontFamily: ['Roboto', 'sans-serif'].join(','),
  },
  palette: {
    primary: {
      main: '#ec407a',
    },
    secondary: {
      main: '#303f9f',
    },
    error: {
      main: red.A400,
    },
    success: {
      main: '#61ac24',
    },
  },
  components: {
    MuiAvatar: {
      styleOverrides: {
        root: {
          color: '#fff',
        },
      },
    },
  },
};

const light = createTheme({
  ...theme,
  palette: {
    ...theme.palette,
    mode: 'light',
  },
});

const dark = createTheme({
  ...theme,
  palette: {
    ...theme.palette,
    primary: {
      main: '#a8377a',
    },
    secondary: {
      main: '#4c6cff',
    },
    mode: 'dark',
  },
});

const useTheme = () => {
  const [session] = useSession();
  const prefersDarkMode = useMediaQuery('(prefers-color-scheme: dark)');
  return (session?.theme === undefined && prefersDarkMode) ||
    session?.theme === 'dark'
    ? dark
    : light;
}

export default useTheme;