const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const devCerts = require('office-addin-dev-certs');

module.exports = async (env, argv) => {
  const devServer =
    argv.mode === 'development'
      ? {
          server: {
            type: 'https',
            options: await devCerts.getHttpsServerOptions(),
          },
          port: 3000,
          hot: true,
          headers: {
            'Access-Control-Allow-Origin': '*',
          },
        }
      : undefined;

  return {
    entry: {
      taskpane: './src/taskpane.ts',
    },
    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: '[name].js',
      clean: true,
    },
    resolve: {
      extensions: ['.ts', '.js'],
    },
    module: {
      rules: [
        {
          test: /\.ts$/,
          use: 'ts-loader',
          exclude: /node_modules/,
        },
      ],
    },
    plugins: [
      new HtmlWebpackPlugin({
        template: './src/taskpane.html',
        filename: 'taskpane.html',
        chunks: ['taskpane'],
      }),
    ],
    devServer,
  };
};
